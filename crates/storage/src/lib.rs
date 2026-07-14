use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};
use domain_model::{
    AssistantRun, AssistantRunEvent, AssistantRunEventId, AssistantRunId, AuthAuditEvent,
    AuthAuditEventId, AuthAuditOutcome, AuthChallengePurpose, AuthSessionMethod, ChatMessage,
    ChatMessageId, ChatMessageRole, ChatSession, ChatSessionId, ConversationMemoryItem,
    ConversationMemoryItemId, Dataset, DatasetId, DatasetLifecycle, DatasetOutput, DatasetOutputId,
    DatasetVisibility, Document, DocumentChunk, DocumentChunkId, DocumentChunkState, DocumentId,
    DocumentLifecycle, EmailVerificationChallenge, EmailVerificationChallengeId, HtmlArtifact,
    LlmInvocation, LlmInvocationFinishReason, LlmInvocationId, LlmInvocationMode,
    LlmInvocationSourceKind, LlmTokenUsage, MemoryDirectory, MemoryDirectoryId, PublishedReport,
    PublishedReportId, PublishedReportVersion, PublishedReportVersionId, PublishedVideoPptPackage,
    PublishedVideoPptPackageId, PublishedVideoPptVersion, PublishedVideoPptVersionId, ReportPlan,
    ReportPlanAstVersion, ReportPlanAstVersionId, ReportPlanId, ReportPlanStatus,
    ReportRenderOutput, ReportRenderOutputId, ReportRenderOutputStatus, RetrievalEvidence,
    RetrievalEvidenceId, SecretBinding, SecretBindingId, SecretScopeLevel, StaticPageDraft,
    StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJob, StaticPageImageJobId,
    StaticPageImageJobStatus, StaticPageRenderOutput, StaticPageRenderOutputId,
    StaticPageRenderOutputStatus, Tenant, TenantId, ToolExecution, ToolExecutionId,
    ToolExecutionSourceKind, ToolExecutionStatus, User, UserId, UserSession, UserSessionId,
    WorkflowEventId, WorkflowEventRecord, WorkflowExecution, WorkflowExecutionId, WorkflowKind,
    WorkflowStatus, WorkflowTask, WorkflowTaskId, WorkflowTaskStatus,
};
use serde_json::{Map, Value};
use sqlx::{postgres::PgPoolOptions, AssertSqlSafe, Executor, PgPool, Row};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use uuid::Uuid;
use workflow_engine::WorkflowDefinitionSummary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Migration {
    pub version: &'static str,
    pub description: &'static str,
    pub sql: &'static str,
}

pub const INITIAL_SCHEMA: Migration = Migration {
    version: "0001",
    description: "initial platform system-of-record schema",
    sql: include_str!("../migrations/0001_initial_schema.sql"),
};

pub const WORKFLOW_RUNTIME_RECORDS_SCHEMA: Migration = Migration {
    version: "0002",
    description: "workflow execution runtime records",
    sql: include_str!("../migrations/0002_workflow_execution_runtime_records.sql"),
};

pub const EMAIL_ACCOUNT_AUTH_SCHEMA: Migration = Migration {
    version: "0004",
    description: "email account authentication",
    sql: include_str!("../migrations/0004_email_account_auth.sql"),
};

pub const ACCOUNT_ARTIFACT_HARDENING_SCHEMA: Migration = Migration {
    version: "0005",
    description: "account artifact hardening",
    sql: include_str!("../migrations/0005_account_artifact_hardening.sql"),
};

pub const MEMORY_DIRECTORY_SCOPE_HARDENING_SCHEMA: Migration = Migration {
    version: "0006",
    description: "memory directory scope hardening",
    sql: include_str!("../migrations/0006_memory_directory_scope_hardening.sql"),
};

pub const HTML_ARTIFACTS_SCHEMA: Migration = Migration {
    version: "0007",
    description: "safe html artifact records",
    sql: include_str!("../migrations/0007_html_artifacts.sql"),
};

pub const EXTERNAL_INTEGRATIONS_SCHEMA: Migration = Migration {
    version: "0008",
    description: "external bot and third-party integrations",
    sql: include_str!("../migrations/0008_external_integrations.sql"),
};

pub const VIDEO_PPT_PUBLISHED_VERSIONS_SCHEMA: Migration = Migration {
    version: "0009",
    description: "video ppt published version history",
    sql: include_str!("../migrations/0009_video_ppt_published_versions.sql"),
};

pub const DATASET_DOCUMENT_MEMBERSHIPS_SCHEMA: Migration = Migration {
    version: "0010",
    description: "dataset document memberships",
    sql: include_str!("../migrations/0010_dataset_document_memberships.sql"),
};

pub const MODEL_GATEWAY_PROFILES_SCHEMA: Migration = Migration {
    version: "0011",
    description: "model gateway profiles",
    sql: include_str!("../migrations/0011_model_gateway_profiles.sql"),
};

pub const DOCUMENT_FACT_INDEX_SCHEMA: Migration = Migration {
    version: "0012",
    description: "document fact index",
    sql: include_str!("../migrations/0012_document_fact_index.sql"),
};

pub const DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA: Migration = Migration {
    version: "0013",
    description: "document canonical enrichment",
    sql: include_str!("../migrations/0013_document_canonical_enrichment.sql"),
};

pub const RETRIEVAL_LEXICAL_INDEX_SCHEMA: Migration = Migration {
    version: "0014",
    description: "retrieval evidence lexical search index",
    sql: include_str!("../migrations/0014_retrieval_lexical_index.sql"),
};

pub const ASSET_LIBRARIES_SCHEMA: Migration = Migration {
    version: "0015",
    description: "enterprise asset libraries",
    sql: include_str!("../migrations/0015_asset_libraries.sql"),
};

pub const V3_CLIENT_ARTIFACTS_SCHEMA: Migration = Migration {
    version: "0016",
    description: "v3 client config packages and uploaded artifacts",
    sql: include_str!("../migrations/0016_v3_client_artifacts.sql"),
};

pub const ASSET_PARSE_RUNS_SCHEMA: Migration = Migration {
    version: "0017",
    description: "asset parse runs",
    sql: include_str!("../migrations/0017_asset_parse_runs.sql"),
};

pub const ASSET_RETRIEVAL_EVIDENCES_SCHEMA: Migration = Migration {
    version: "0018",
    description: "asset retrieval evidences",
    sql: include_str!("../migrations/0018_asset_retrieval_evidences.sql"),
};

pub const DATASET_SEMANTIC_UNDERSTANDING_SCHEMA: Migration = Migration {
    version: "0019",
    description: "dataset semantic understanding snapshots and dictionary",
    sql: include_str!("../migrations/0019_dataset_semantic_understanding.sql"),
};

pub const DATASET_SEMANTIC_CROSS_GRAPH_SCHEMA: Migration = Migration {
    version: "0020",
    description: "dataset semantic cross-graph link snapshots and runs",
    sql: include_str!("../migrations/0020_dataset_semantic_cross_graph.sql"),
};

pub const MIGRATIONS: &[Migration] = &[
    INITIAL_SCHEMA,
    WORKFLOW_RUNTIME_RECORDS_SCHEMA,
    EMAIL_ACCOUNT_AUTH_SCHEMA,
    ACCOUNT_ARTIFACT_HARDENING_SCHEMA,
    MEMORY_DIRECTORY_SCOPE_HARDENING_SCHEMA,
    HTML_ARTIFACTS_SCHEMA,
    EXTERNAL_INTEGRATIONS_SCHEMA,
    VIDEO_PPT_PUBLISHED_VERSIONS_SCHEMA,
    DATASET_DOCUMENT_MEMBERSHIPS_SCHEMA,
    MODEL_GATEWAY_PROFILES_SCHEMA,
    DOCUMENT_FACT_INDEX_SCHEMA,
    DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA,
    RETRIEVAL_LEXICAL_INDEX_SCHEMA,
    ASSET_LIBRARIES_SCHEMA,
    V3_CLIENT_ARTIFACTS_SCHEMA,
    ASSET_PARSE_RUNS_SCHEMA,
    ASSET_RETRIEVAL_EVIDENCES_SCHEMA,
    DATASET_SEMANTIC_UNDERSTANDING_SCHEMA,
    DATASET_SEMANTIC_CROSS_GRAPH_SCHEMA,
];

pub const TABLES: &[&str] = &[
    "tenants",
    "users",
    "user_sessions",
    "email_verification_challenges",
    "auth_audit_events",
    "datasets",
    "documents",
    "dataset_document_memberships",
    "document_chunks",
    "document_facts",
    "document_fact_sources",
    "dataset_fact_snapshots",
    "dataset_semantic_snapshots",
    "dataset_semantic_link_snapshots",
    "dataset_semantic_link_runs",
    "semantic_dictionary_entries",
    "document_content_fingerprints",
    "document_enrichment_runs",
    "secret_bindings",
    "secret_grants",
    "workflow_definitions",
    "workflow_executions",
    "workflow_events",
    "workflow_tasks",
    "prompt_definitions",
    "prompt_versions",
    "tool_definitions",
    "tool_versions",
    "report_plans",
    "report_plan_ast_versions",
    "report_render_outputs",
    "memory_directories",
    "retrieval_evidences",
    "dataset_outputs",
    "assistant_run_events",
    "assistant_runs",
    "conversation_memory_items",
    "html_artifacts",
    "external_channel_connections",
    "external_source_connections",
    "external_principals",
    "external_permission_snapshots",
    "external_message_events",
    "external_action_runs",
    "external_sync_runs",
    "model_gateway_profiles",
    "model_gateway_profile_events",
    "published_video_ppt_packages",
    "published_video_ppt_versions",
    "static_page_drafts",
    "static_page_image_jobs",
    "static_page_render_outputs",
    "chat_sessions",
    "chat_messages",
    "llm_invocations",
    "tool_executions",
    "report_modules",
    "published_reports",
    "published_report_versions",
    "enterprise_asset_libraries",
    "asset_library_dataset_memberships",
    "asset_collections",
    "asset_items",
    "asset_parse_runs",
    "dataset_asset_memberships",
    "asset_profiles",
    "asset_retrieval_evidences",
    "v3_client_config_packages",
    "v3_client_artifacts",
    "v3_client_artifact_files",
];

pub const DEFAULT_LOCAL_DATABASE_URL: &str =
    "postgres://ai_platform:ai_platform@127.0.0.1:5432/ai_data_platform_v3";
pub const DEFAULT_LOCAL_TENANT_KEY: &str = "local-dev";
pub const DEFAULT_LOCAL_TENANT_NAME: &str = "Local Development";
pub const DEFAULT_DATABASE_MAX_CONNECTIONS: u32 = 10;
pub const CONFIGURED_DATABASE_MAX_CONNECTIONS_LIMIT: u32 = 100;

#[derive(Clone, Debug)]
pub struct NewDataset {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub owner_user_id: Option<UserId>,
}

#[derive(Clone, Debug)]
pub struct AssetLibraryRecord {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub external_id: Option<String>,
    pub name: String,
    pub domain: String,
    pub description: Option<String>,
    pub visibility: String,
    pub metadata: Value,
    pub dataset_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewAssetLibrary {
    pub external_id: Option<String>,
    pub name: String,
    pub domain: String,
    pub description: Option<String>,
    pub visibility: String,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct AssetLibraryDatasetMembershipRecord {
    pub tenant_id: TenantId,
    pub asset_library_id: Uuid,
    pub dataset_id: DatasetId,
    pub role: String,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewAssetLibraryDatasetMembership {
    pub dataset_id: DatasetId,
    pub role: String,
    pub priority: i32,
}

#[derive(Clone, Debug)]
pub struct AssetItemRecord {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub asset_library_id: Option<Uuid>,
    pub collection_id: Option<Uuid>,
    pub external_id: Option<String>,
    pub title: String,
    pub asset_kind: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub content_type: Option<String>,
    pub object_key: Option<String>,
    pub metadata: Value,
    pub profile_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewAssetItem {
    pub asset_library_id: Option<Uuid>,
    pub collection_id: Option<Uuid>,
    pub external_id: Option<String>,
    pub title: String,
    pub asset_kind: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub content_type: Option<String>,
    pub object_key: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct DatasetAssetMembershipRecord {
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub asset_id: Uuid,
    pub membership_kind: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDatasetAssetMembership {
    pub dataset_id: DatasetId,
    pub membership_kind: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct AssetParseRunRecord {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub asset_id: Uuid,
    pub parser_name: String,
    pub parser_version: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewAssetParseRun {
    pub parser_name: String,
    pub parser_version: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct AssetProfileRecord {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub asset_id: Uuid,
    pub profile_kind: String,
    pub profile_version: String,
    pub attributes: Value,
    pub embedding_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewAssetProfile {
    pub profile_kind: String,
    pub profile_version: String,
    pub attributes: Value,
    pub embedding_status: String,
}

#[derive(Clone, Debug)]
pub struct SyncedDocumentAssetProfile {
    pub asset: AssetItemRecord,
    pub dataset_membership: DatasetAssetMembershipRecord,
    pub profile: AssetProfileRecord,
}

#[derive(Clone, Debug)]
pub struct NewDocument {
    pub dataset_id: DatasetId,
    pub title: String,
    pub object_key: String,
    pub content_type: String,
    pub secret_binding_ids: Vec<SecretBindingId>,
    pub owner_user_id: Option<UserId>,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct NewDocumentEnrichmentRun {
    pub document_id: DocumentId,
    pub enrichment_kind: String,
    pub parse_version: Option<String>,
    pub input_fingerprint: String,
    pub priority: i32,
    pub max_attempts: i32,
    pub available_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct DocumentEnrichmentRun {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub document_id: DocumentId,
    pub enrichment_kind: String,
    pub parse_version: Option<String>,
    pub input_fingerprint: String,
    pub status: String,
    pub priority: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub available_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub output_summary: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDatasetDocumentMembership {
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub membership_kind: String,
    pub source: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetDocumentMembership {
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub membership_kind: String,
    pub source: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct ModelGatewayProfile {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub profile_id: String,
    pub display_name: String,
    pub lane: String,
    pub provider_id: String,
    pub model_id: String,
    pub base_url: Option<String>,
    pub api_path: Option<String>,
    pub wire_api: String,
    pub auth_mode: String,
    pub auth_env_key_name: Option<String>,
    pub recommended_preset: Option<String>,
    pub max_concurrency: Option<i32>,
    pub rpm_limit: Option<i32>,
    pub tpm_limit: Option<i32>,
    pub timeout_ms: Option<i32>,
    pub priority: i32,
    pub enabled: bool,
    pub capabilities: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewModelGatewayProfile {
    pub id: Uuid,
    pub profile_id: String,
    pub display_name: String,
    pub lane: String,
    pub provider_id: String,
    pub model_id: String,
    pub base_url: Option<String>,
    pub api_path: Option<String>,
    pub wire_api: String,
    pub auth_mode: String,
    pub auth_env_key_name: Option<String>,
    pub recommended_preset: Option<String>,
    pub max_concurrency: Option<i32>,
    pub rpm_limit: Option<i32>,
    pub tpm_limit: Option<i32>,
    pub timeout_ms: Option<i32>,
    pub priority: i32,
    pub enabled: bool,
    pub capabilities: Value,
}

#[derive(Clone, Debug, Default)]
pub struct ModelGatewayProfileUpdate {
    pub display_name: Option<String>,
    pub lane: Option<String>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub base_url: Option<String>,
    pub api_path: Option<String>,
    pub wire_api: Option<String>,
    pub auth_mode: Option<String>,
    pub auth_env_key_name: Option<String>,
    pub recommended_preset: Option<String>,
    pub max_concurrency: Option<i32>,
    pub rpm_limit: Option<i32>,
    pub tpm_limit: Option<i32>,
    pub timeout_ms: Option<i32>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
    pub capabilities: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct NewModelGatewayProfileEvent {
    pub profile_id: String,
    pub lane: String,
    pub event_type: String,
    pub latency_ms: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub error_kind: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelGatewayProfileUsageSummary {
    pub profile_id: String,
    pub lane: String,
    pub request_count: i64,
    pub success_count: i64,
    pub failure_count: i64,
    pub timeout_count: i64,
    pub rate_limit_count: i64,
    pub would_throttle_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub shadow_eval_count: i64,
    pub shadow_eval_pass_count: i64,
    pub shadow_eval_fail_count: i64,
    pub shadow_eval_format_pass_count: i64,
    pub shadow_eval_repair_count: i64,
    pub last_shadow_eval_at: Option<DateTime<Utc>>,
    pub last_profile_test_status: Option<String>,
    pub last_profile_test_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct NewSecretBinding {
    pub dataset_id: DatasetId,
    pub document_id: Option<DocumentId>,
    pub scope_level: SecretScopeLevel,
    pub provider_key: String,
    pub cipher_text: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug)]
pub struct NewEmailVerificationChallenge {
    pub email_normalized: String,
    pub purpose: AuthChallengePurpose,
    pub code_hash: String,
    pub max_attempts: i32,
    pub expires_at: DateTime<Utc>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewUserSession {
    pub user_id: UserId,
    pub device_fingerprint: String,
    pub session_token_hash: String,
    pub auth_method: AuthSessionMethod,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewAuthAuditEvent {
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

#[derive(Clone, Debug)]
pub struct NewDocumentChunk {
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub chunk_index: i32,
    pub content: String,
    pub token_count: i32,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDocumentFactSource {
    pub source_kind: String,
    pub source_locator: Option<String>,
    pub source_chunk_id: Option<DocumentChunkId>,
    pub attributes: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDocumentFact {
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub fact_type: String,
    pub name: String,
    pub normalized_name: String,
    pub value_text: Option<String>,
    pub value_number: Option<f64>,
    pub value_date: Option<NaiveDate>,
    pub attributes: Value,
    pub confidence: f64,
    pub source_kind: String,
    pub source_locator: Option<String>,
    pub source_chunk_id: Option<DocumentChunkId>,
    pub parse_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub sources: Vec<NewDocumentFactSource>,
}

#[derive(Clone, Debug)]
pub struct DocumentFact {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub fact_type: String,
    pub name: String,
    pub normalized_name: String,
    pub value_text: Option<String>,
    pub value_number: Option<f64>,
    pub value_date: Option<NaiveDate>,
    pub attributes: Value,
    pub confidence: f64,
    pub source_kind: String,
    pub source_locator: Option<String>,
    pub source_chunk_id: Option<DocumentChunkId>,
    pub parse_version: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct DocumentFactAggregate {
    pub fact_type: String,
    pub normalized_name: String,
    pub name: String,
    pub fact_count: i64,
    pub document_count: i64,
    pub source_document_ids: Vec<DocumentId>,
    pub source_locators: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DatasetFactSnapshot {
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub snapshot_kind: String,
    pub snapshot_key: String,
    pub snapshot_manifest: Value,
    pub source_fact_count: i64,
    pub source_document_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDatasetSemanticSnapshot {
    pub dataset_id: DatasetId,
    pub schema_version: String,
    pub generation_version: String,
    pub source_fingerprint: String,
    pub manifest: Value,
    pub source_document_count: i64,
    pub source_asset_count: i64,
    pub source_record_count: i64,
}

#[derive(Clone, Debug)]
pub struct DatasetSemanticSnapshot {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub schema_version: String,
    pub generation_version: String,
    pub source_fingerprint: String,
    pub status: String,
    pub manifest: Value,
    pub source_document_count: i64,
    pub source_asset_count: i64,
    pub source_record_count: i64,
    pub node_count: i32,
    pub edge_count: i32,
    pub failure_code: Option<String>,
    pub generated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDatasetSemanticLinkSnapshot {
    pub left_dataset_id: DatasetId,
    pub right_dataset_id: DatasetId,
    pub left_snapshot_id: Uuid,
    pub right_snapshot_id: Uuid,
    pub schema_version: String,
    pub generation_version: String,
    pub source_fingerprint: String,
    pub manifest: Value,
}

#[derive(Clone, Debug)]
pub struct DatasetSemanticLinkSnapshot {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub left_dataset_id: DatasetId,
    pub right_dataset_id: DatasetId,
    pub left_snapshot_id: Uuid,
    pub right_snapshot_id: Uuid,
    pub schema_version: String,
    pub generation_version: String,
    pub source_fingerprint: String,
    pub status: String,
    pub manifest: Value,
    pub node_count: i32,
    pub edge_count: i32,
    pub failure_code: Option<String>,
    pub generated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDatasetSemanticLinkRun {
    pub left_dataset_id: DatasetId,
    pub right_dataset_id: DatasetId,
    pub left_snapshot_id: Uuid,
    pub right_snapshot_id: Uuid,
    pub generation_version: String,
    pub source_fingerprint: String,
    pub priority: i32,
    pub max_attempts: i32,
    pub available_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct DatasetSemanticLinkRun {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub left_dataset_id: DatasetId,
    pub right_dataset_id: DatasetId,
    pub left_snapshot_id: Uuid,
    pub right_snapshot_id: Uuid,
    pub generation_version: String,
    pub source_fingerprint: String,
    pub status: String,
    pub priority: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub available_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub failure_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewSemanticDictionaryEntry {
    pub source_kind: String,
    pub source_system_key: String,
    pub source_object_key: String,
    pub raw_field_key: String,
    pub display_name: String,
    pub description: Option<String>,
    pub semantic_role: String,
    pub value_type: String,
    pub status: String,
    pub confidence: f64,
    pub created_by_user_id: Option<UserId>,
}

#[derive(Clone, Debug)]
pub struct SemanticDictionaryEntry {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub source_kind: String,
    pub source_system_key: String,
    pub source_object_key: String,
    pub raw_field_key: String,
    pub display_name: String,
    pub description: Option<String>,
    pub semantic_role: String,
    pub value_type: String,
    pub status: String,
    pub confidence: f64,
    pub created_by_user_id: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewReportPlan {
    pub dataset_id: DatasetId,
    pub title: String,
    pub objective: String,
    pub theme_key: String,
    pub owner_user_id: Option<UserId>,
}

#[derive(Clone, Debug)]
pub struct NewWorkflowTask {
    pub queue: String,
    pub task_key: String,
    pub payload: Value,
    pub available_at: DateTime<Utc>,
    pub max_attempts: u32,
}

#[derive(Clone, Debug)]
pub struct NewReportPlanAstVersion {
    pub ast: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewReportRenderOutput {
    pub execution_id: WorkflowExecutionId,
    pub plan_id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub ast_version_id: ReportPlanAstVersionId,
    pub surface: domain_model::PublishedSurface,
    pub status: ReportRenderOutputStatus,
    pub asset_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewPublishedReport {
    pub dataset_id: DatasetId,
    pub plan_id: ReportPlanId,
    pub slug: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewPublishedReportVersion {
    pub surface: domain_model::PublishedSurface,
    pub asset_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewPublishedVideoPptVersion {
    pub assistant_run_id: AssistantRunId,
    pub document_id: DocumentId,
    pub dataset_id: DatasetId,
    pub package_key: String,
    pub version_fingerprint: String,
    pub lifecycle_state: String,
    pub artifact_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewMemoryDirectory {
    pub execution_id: WorkflowExecutionId,
    pub dataset_id: DatasetId,
    pub owner_user_id: Option<UserId>,
    pub source_document_ids: Vec<DocumentId>,
    pub directory_nodes: i32,
    pub refreshed_chunks: i32,
    pub directory_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDatasetOutput {
    pub execution_id: WorkflowExecutionId,
    pub dataset_id: DatasetId,
    pub owner_user_id: Option<UserId>,
    pub prompt: String,
    pub output_text: String,
    pub memory_directory_id: Option<MemoryDirectoryId>,
    pub retrieval_evidence_ids: Vec<RetrievalEvidenceId>,
    pub output_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewAssistantRun {
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
}

#[derive(Clone, Debug)]
pub struct NewAssistantRunEvent {
    pub event_name: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewHtmlArtifact {
    pub id: String,
    pub owner_user_id: Option<UserId>,
    pub assistant_run_id: Option<AssistantRunId>,
    pub local_thread_id: Option<String>,
    pub source_type: String,
    pub template_id: String,
    pub interaction_mode: String,
    pub manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewConversationMemoryItem {
    pub user_id: Option<UserId>,
    pub local_thread_id: String,
    pub role: ChatMessageRole,
    pub item_kind: String,
    pub summary: String,
    pub source_message_refs: Value,
    pub artifact_refs: Value,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewStaticPageDraft {
    pub assistant_run_id: AssistantRunId,
    pub owner_user_id: Option<UserId>,
    pub title: String,
    pub status: StaticPageDraftStatus,
    pub selected_scope: Value,
    pub visibility_snapshot: Value,
    pub source_refs: Value,
    pub draft_payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewStaticPageImageJob {
    pub draft_id: StaticPageDraftId,
    pub assistant_run_id: AssistantRunId,
    pub status: StaticPageImageJobStatus,
    pub queue_position: Option<i32>,
    pub image_prompt_payload: Value,
    pub preview_asset_key: Option<String>,
    pub failure_reason: Option<String>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewStaticPageRenderOutput {
    pub draft_id: StaticPageDraftId,
    pub assistant_run_id: AssistantRunId,
    pub owner_user_id: Option<UserId>,
    pub image_job_id: Option<StaticPageImageJobId>,
    pub status: StaticPageRenderOutputStatus,
    pub html: String,
    pub asset_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewRetrievalEvidence {
    pub execution_id: WorkflowExecutionId,
    pub dataset_id: DatasetId,
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

#[derive(Clone, Debug)]
pub struct LexicalRetrievalQuery {
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub query: String,
    pub document_ids: Vec<DocumentId>,
    pub owner_user_id: Option<UserId>,
    pub limit: usize,
    pub candidate_limit: usize,
}

#[derive(Clone, Debug)]
pub struct AssetRetrievalEvidenceRecord {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub asset_id: Uuid,
    pub asset_profile_id: Uuid,
    pub profile_kind: String,
    pub profile_version: String,
    pub materialized_text: String,
    pub safe_metadata: Value,
    pub content_hash: String,
    pub search_terms: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewAssetRetrievalEvidence {
    pub dataset_id: DatasetId,
    pub asset_id: Uuid,
    pub asset_profile_id: Uuid,
    pub profile_kind: String,
    pub profile_version: String,
    pub materialized_text: String,
    pub safe_metadata: Value,
    pub content_hash: String,
    pub search_terms: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct AssetRetrievalEvidenceSearchQuery {
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub query: String,
    pub limit: usize,
}

#[derive(Clone, Debug)]
pub struct NewChatSession {
    pub id: ChatSessionId,
    pub execution_id: WorkflowExecutionId,
    pub dataset_id: DatasetId,
    pub user_id: Option<UserId>,
    pub title: String,
    pub latest_memory_directory_id: Option<MemoryDirectoryId>,
    pub latest_dataset_output_id: Option<DatasetOutputId>,
    pub session_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewChatMessage {
    pub session_id: ChatSessionId,
    pub role: ChatMessageRole,
    pub turn_index: i32,
    pub content: String,
    pub message_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct LlmInvocationRecordInput {
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
}

#[derive(Clone, Debug)]
pub struct ToolExecutionRecordInput {
    pub call_id: Option<String>,
    pub tool_name: String,
    pub tool_snapshot: Option<Value>,
    pub status: ToolExecutionStatus,
    pub arguments: Option<Value>,
    pub result: Option<Value>,
}

#[derive(Clone)]
pub struct PgStorage {
    pool: PgPool,
}

impl PgStorage {
    pub async fn connect(database_url: &str) -> Result<Self> {
        Self::connect_with_settings(
            database_url,
            DEFAULT_DATABASE_MAX_CONNECTIONS,
            Duration::from_secs(30),
        )
        .await
    }

    pub async fn connect_with_configured_max_connections(
        database_url: &str,
        service_env_key: &str,
    ) -> Result<Self> {
        Self::connect_with_max_connections(
            database_url,
            configured_database_max_connections(service_env_key),
        )
        .await
    }

    pub async fn connect_with_max_connections(
        database_url: &str,
        max_connections: u32,
    ) -> Result<Self> {
        Self::connect_with_settings(database_url, max_connections, Duration::from_secs(30)).await
    }

    pub async fn connect_with_settings(
        database_url: &str,
        max_connections: u32,
        acquire_timeout: Duration,
    ) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(acquire_timeout)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("select 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn migrate(&self) -> Result<()> {
        for migration in MIGRATIONS {
            sqlx::raw_sql(migration.sql).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn sync_workflow_definitions(
        &self,
        definitions: &[WorkflowDefinitionSummary],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for definition in definitions {
            sqlx::query(
                r#"
                update workflow_definitions
                set is_active = false
                where kind = $1 and version <> $2
                "#,
            )
            .bind(definition.kind.as_str())
            .bind(&definition.version)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                insert into workflow_definitions (kind, version, summary, is_active, metadata)
                values ($1, $2, $3, true, '{}'::jsonb)
                on conflict (kind, version) do update
                set summary = excluded.summary,
                    is_active = true
                "#,
            )
            .bind(definition.kind.as_str())
            .bind(&definition.version)
            .bind(&definition.summary)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn ensure_tenant(&self, key: &str, name: &str) -> Result<Tenant> {
        let row = sqlx::query(
            r#"
            insert into tenants (key, name)
            values ($1, $2)
            on conflict (key) do update
            set name = excluded.name
            returning id, key, name, created_at
            "#,
        )
        .bind(key)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(Tenant {
            id: TenantId(row.get::<Uuid, _>("id")),
            key: row.get("key"),
            name: row.get("name"),
            created_at: row.get("created_at"),
        })
    }

    pub fn datasets(&self) -> PgDatasetRepository {
        PgDatasetRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn asset_libraries(&self) -> PgAssetLibraryRepository {
        PgAssetLibraryRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn asset_items(&self) -> PgAssetItemRepository {
        PgAssetItemRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn documents(&self) -> PgDocumentRepository {
        PgDocumentRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn document_enrichment_runs(&self) -> PgDocumentEnrichmentRunRepository {
        PgDocumentEnrichmentRunRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn dataset_document_memberships(&self) -> PgDatasetDocumentMembershipRepository {
        PgDatasetDocumentMembershipRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn model_gateway_profiles(&self) -> PgModelGatewayProfileRepository {
        PgModelGatewayProfileRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn secret_bindings(&self) -> PgSecretBindingRepository {
        PgSecretBindingRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn users(&self) -> PgUserRepository {
        PgUserRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn user_sessions(&self) -> PgUserSessionRepository {
        PgUserSessionRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn email_verification_challenges(&self) -> PgEmailVerificationChallengeRepository {
        PgEmailVerificationChallengeRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn auth_audit_events(&self) -> PgAuthAuditEventRepository {
        PgAuthAuditEventRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn document_chunks(&self) -> PgDocumentChunkRepository {
        PgDocumentChunkRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn document_facts(&self) -> PgDocumentFactRepository {
        PgDocumentFactRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn dataset_fact_snapshots(&self) -> PgDatasetFactSnapshotRepository {
        PgDatasetFactSnapshotRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn dataset_semantic_snapshots(&self) -> PgDatasetSemanticSnapshotRepository {
        PgDatasetSemanticSnapshotRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn dataset_semantic_links(&self) -> PgDatasetSemanticLinkRepository {
        PgDatasetSemanticLinkRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn semantic_dictionary_entries(&self) -> PgSemanticDictionaryRepository {
        PgSemanticDictionaryRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn report_plans(&self) -> PgReportPlanRepository {
        PgReportPlanRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn report_plan_ast_versions(&self) -> PgReportPlanAstVersionRepository {
        PgReportPlanAstVersionRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn report_render_outputs(&self) -> PgReportRenderOutputRepository {
        PgReportRenderOutputRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn published_reports(&self) -> PgPublishedReportRepository {
        PgPublishedReportRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn published_report_versions(&self) -> PgPublishedReportVersionRepository {
        PgPublishedReportVersionRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn published_video_ppt_versions(&self) -> PgPublishedVideoPptVersionRepository {
        PgPublishedVideoPptVersionRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn memory_directories(&self) -> PgMemoryDirectoryRepository {
        PgMemoryDirectoryRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn retrieval_evidences(&self) -> PgRetrievalEvidenceRepository {
        PgRetrievalEvidenceRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn asset_retrieval_evidences(&self) -> PgAssetRetrievalEvidenceRepository {
        PgAssetRetrievalEvidenceRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn dataset_outputs(&self) -> PgDatasetOutputRepository {
        PgDatasetOutputRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn assistant_runs(&self) -> PgAssistantRunRepository {
        PgAssistantRunRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn conversation_memory_items(&self) -> PgConversationMemoryItemRepository {
        PgConversationMemoryItemRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn html_artifacts(&self) -> PgHtmlArtifactRepository {
        PgHtmlArtifactRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn static_page_drafts(&self) -> PgStaticPageDraftRepository {
        PgStaticPageDraftRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn static_page_image_jobs(&self) -> PgStaticPageImageJobRepository {
        PgStaticPageImageJobRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn static_page_render_outputs(&self) -> PgStaticPageRenderOutputRepository {
        PgStaticPageRenderOutputRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn chat_sessions(&self) -> PgChatSessionRepository {
        PgChatSessionRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn chat_messages(&self) -> PgChatMessageRepository {
        PgChatMessageRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn llm_invocations(&self) -> PgLlmInvocationRepository {
        PgLlmInvocationRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn tool_executions(&self) -> PgToolExecutionRepository {
        PgToolExecutionRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn workflow_executions(&self) -> PgWorkflowExecutionRepository {
        PgWorkflowExecutionRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn workflow_events(&self) -> PgWorkflowEventRepository {
        PgWorkflowEventRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn workflow_tasks(&self) -> PgWorkflowTaskRepository {
        PgWorkflowTaskRepository {
            pool: self.pool.clone(),
        }
    }
}

pub fn configured_database_max_connections(service_env_key: &str) -> u32 {
    let service_value = std::env::var(service_env_key).ok();
    let global_value = std::env::var("PLATFORM_DATABASE_MAX_CONNECTIONS").ok();
    parse_configured_database_max_connections(service_value.as_deref(), global_value.as_deref())
}

pub fn parse_configured_database_max_connections(
    service_value: Option<&str>,
    global_value: Option<&str>,
) -> u32 {
    service_value
        .and_then(parse_positive_database_max_connections)
        .or_else(|| global_value.and_then(parse_positive_database_max_connections))
        .unwrap_or(DEFAULT_DATABASE_MAX_CONNECTIONS)
}

fn parse_positive_database_max_connections(value: &str) -> Option<u32> {
    value
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value.min(CONFIGURED_DATABASE_MAX_CONNECTIONS_LIMIT))
}

#[derive(Clone)]
pub struct PgDatasetRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgAssetLibraryRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgAssetItemRepository {
    pool: PgPool,
}

impl PgDatasetRepository {
    pub async fn create(&self, tenant_id: TenantId, new_dataset: NewDataset) -> Result<Dataset> {
        self.create_with_metadata(tenant_id, new_dataset, Value::Null)
            .await
    }

    pub async fn create_with_metadata(
        &self,
        tenant_id: TenantId,
        new_dataset: NewDataset,
        metadata: Value,
    ) -> Result<Dataset> {
        let row = sqlx::query(
            r#"
            insert into datasets (tenant_id, owner_user_id, key, title, description, lifecycle, metadata)
            values ($1, $2, $3, $4, $5, $6, $7)
            returning id, tenant_id, owner_user_id, key, title, description, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_dataset.owner_user_id.map(|id| id.0))
        .bind(new_dataset.key)
        .bind(new_dataset.title)
        .bind(new_dataset.description)
        .bind(DatasetLifecycle::Draft.as_str())
        .bind(dataset_initial_metadata(&metadata)?)
        .fetch_one(&self.pool)
        .await?;

        map_dataset_row(&row)
    }

    pub async fn list_by_tenant(&self, tenant_id: TenantId) -> Result<Vec<Dataset>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, key, title, description, lifecycle, metadata, created_at, updated_at
            from datasets
            where tenant_id = $1
            order by created_at desc, key asc
            "#,
        )
        .bind(tenant_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_dataset_row).collect()
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Option<Dataset>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, key, title, description, lifecycle, metadata, created_at, updated_at
            from datasets
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_dataset_row).transpose()
    }

    pub async fn get_by_key(&self, tenant_id: TenantId, key: &str) -> Result<Option<Dataset>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, key, title, description, lifecycle, metadata, created_at, updated_at
            from datasets
            where tenant_id = $1 and key = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_dataset_row).transpose()
    }

    pub async fn update_metadata(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        metadata_updates: &Value,
    ) -> Result<Dataset> {
        let current = self
            .get_by_id(tenant_id, dataset_id)
            .await?
            .ok_or_else(|| anyhow!("dataset {dataset_id} was not found"))?;
        let merged_metadata = merge_dataset_metadata(&current.metadata, metadata_updates)?;
        let row = sqlx::query(
            r#"
            update datasets
            set metadata = $3,
                updated_at = now()
            where tenant_id = $1 and id = $2
            returning id, tenant_id, owner_user_id, key, title, description, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(merged_metadata)
        .fetch_one(&self.pool)
        .await?;

        map_dataset_row(&row)
    }

    pub async fn update_state(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        title: Option<&str>,
        description: Option<&str>,
        lifecycle: Option<DatasetLifecycle>,
        metadata_updates: &Value,
    ) -> Result<Dataset> {
        let current = self
            .get_by_id(tenant_id, dataset_id)
            .await?
            .ok_or_else(|| anyhow!("dataset {dataset_id} was not found"))?;
        let merged_metadata = merge_dataset_metadata(&current.metadata, metadata_updates)?;
        let next_title = title.unwrap_or(&current.title).to_string();
        let next_description = description
            .map(ToString::to_string)
            .or_else(|| current.description.clone());
        let next_lifecycle = lifecycle.unwrap_or(current.lifecycle);

        let row = sqlx::query(
            r#"
            update datasets
            set title = $3,
                description = $4,
                lifecycle = $5,
                metadata = $6,
                updated_at = now()
            where tenant_id = $1 and id = $2
            returning id, tenant_id, owner_user_id, key, title, description, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(next_title)
        .bind(next_description)
        .bind(next_lifecycle.as_str())
        .bind(merged_metadata)
        .fetch_one(&self.pool)
        .await?;

        map_dataset_row(&row)
    }

    pub async fn update_owner_user_id(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        owner_user_id: UserId,
    ) -> Result<Option<Dataset>> {
        let row = sqlx::query(
            r#"
            update datasets
            set owner_user_id = $3,
                updated_at = now()
            where tenant_id = $1
              and id = $2
              and (owner_user_id is null or owner_user_id = $3)
            returning id, tenant_id, owner_user_id, key, title, description, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(owner_user_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_dataset_row).transpose()
    }
}

impl PgAssetLibraryRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_asset_library: NewAssetLibrary,
    ) -> Result<AssetLibraryRecord> {
        let row = sqlx::query(
            r#"
            insert into enterprise_asset_libraries (
                tenant_id,
                external_id,
                name,
                domain,
                description,
                visibility,
                metadata
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            returning id, tenant_id, external_id, name, domain, description, visibility, metadata,
                      0::bigint as dataset_count, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_asset_library.external_id)
        .bind(new_asset_library.name)
        .bind(new_asset_library.domain)
        .bind(new_asset_library.description)
        .bind(new_asset_library.visibility)
        .bind(new_asset_library.metadata)
        .fetch_one(&self.pool)
        .await?;

        map_asset_library_row(&row)
    }

    pub async fn list_by_tenant(&self, tenant_id: TenantId) -> Result<Vec<AssetLibraryRecord>> {
        let rows = sqlx::query(
            r#"
            select l.id,
                   l.tenant_id,
                   l.external_id,
                   l.name,
                   l.domain,
                   l.description,
                   l.visibility,
                   l.metadata,
                   (
                       select count(*)
                       from asset_library_dataset_memberships m
                       where m.tenant_id = l.tenant_id
                         and m.asset_library_id = l.id
                   )::bigint as dataset_count,
                   l.created_at,
                   l.updated_at
            from enterprise_asset_libraries l
            where l.tenant_id = $1
            order by l.created_at desc, l.name asc
            "#,
        )
        .bind(tenant_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_asset_library_row).collect()
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        asset_library_id: Uuid,
    ) -> Result<Option<AssetLibraryRecord>> {
        let row = sqlx::query(
            r#"
            select l.id,
                   l.tenant_id,
                   l.external_id,
                   l.name,
                   l.domain,
                   l.description,
                   l.visibility,
                   l.metadata,
                   (
                       select count(*)
                       from asset_library_dataset_memberships m
                       where m.tenant_id = l.tenant_id
                         and m.asset_library_id = l.id
                   )::bigint as dataset_count,
                   l.created_at,
                   l.updated_at
            from enterprise_asset_libraries l
            where l.tenant_id = $1 and l.id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_library_id)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_asset_library_row).transpose()
    }

    pub async fn upsert_dataset_membership(
        &self,
        tenant_id: TenantId,
        asset_library_id: Uuid,
        new_membership: NewAssetLibraryDatasetMembership,
    ) -> Result<AssetLibraryDatasetMembershipRecord> {
        let row = sqlx::query(
            r#"
            insert into asset_library_dataset_memberships (
                tenant_id,
                asset_library_id,
                dataset_id,
                role,
                priority
            )
            values ($1, $2, $3, $4, $5)
            on conflict (tenant_id, asset_library_id, dataset_id) do update
            set role = excluded.role,
                priority = excluded.priority
            returning tenant_id, asset_library_id, dataset_id, role, priority, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_library_id)
        .bind(new_membership.dataset_id.0)
        .bind(new_membership.role)
        .bind(new_membership.priority)
        .fetch_one(&self.pool)
        .await?;

        Ok(map_asset_library_dataset_membership_row(&row))
    }

    pub async fn remove_dataset_membership(
        &self,
        tenant_id: TenantId,
        asset_library_id: Uuid,
        dataset_id: DatasetId,
    ) -> Result<Option<AssetLibraryDatasetMembershipRecord>> {
        let row = sqlx::query(
            r#"
            delete from asset_library_dataset_memberships
            where tenant_id = $1 and asset_library_id = $2 and dataset_id = $3
            returning tenant_id, asset_library_id, dataset_id, role, priority, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_library_id)
        .bind(dataset_id.0)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(map_asset_library_dataset_membership_row))
    }

    pub async fn list_dataset_memberships(
        &self,
        tenant_id: TenantId,
        asset_library_id: Uuid,
    ) -> Result<Vec<AssetLibraryDatasetMembershipRecord>> {
        let rows = sqlx::query(
            r#"
            select tenant_id, asset_library_id, dataset_id, role, priority, created_at
            from asset_library_dataset_memberships
            where tenant_id = $1 and asset_library_id = $2
            order by priority asc, created_at asc, dataset_id asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_library_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(map_asset_library_dataset_membership_row)
            .collect())
    }

    pub async fn get_collection_asset_library_id(
        &self,
        tenant_id: TenantId,
        collection_id: Uuid,
    ) -> Result<Option<Uuid>> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            select asset_library_id
            from asset_collections
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(collection_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }
}

impl PgAssetItemRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_asset: NewAssetItem,
    ) -> Result<AssetItemRecord> {
        let row = sqlx::query(
            r#"
            insert into asset_items (
                tenant_id,
                asset_library_id,
                collection_id,
                external_id,
                title,
                asset_kind,
                source_kind,
                source_id,
                content_type,
                object_key,
                metadata
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            returning id, tenant_id, asset_library_id, collection_id, external_id, title,
                      asset_kind, source_kind, source_id, content_type, object_key, metadata,
                      0::bigint as profile_count, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_asset.asset_library_id)
        .bind(new_asset.collection_id)
        .bind(new_asset.external_id)
        .bind(new_asset.title)
        .bind(new_asset.asset_kind)
        .bind(new_asset.source_kind)
        .bind(new_asset.source_id)
        .bind(new_asset.content_type)
        .bind(new_asset.object_key)
        .bind(new_asset.metadata)
        .fetch_one(&self.pool)
        .await?;

        map_asset_item_row(&row)
    }

    pub async fn upsert_by_source(
        &self,
        tenant_id: TenantId,
        new_asset: NewAssetItem,
    ) -> Result<AssetItemRecord> {
        let row = sqlx::query(
            r#"
            insert into asset_items (
                tenant_id,
                asset_library_id,
                collection_id,
                external_id,
                title,
                asset_kind,
                source_kind,
                source_id,
                content_type,
                object_key,
                metadata
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            on conflict (tenant_id, source_kind, source_id) where source_id is not null do update
            set asset_library_id = coalesce(excluded.asset_library_id, asset_items.asset_library_id),
                collection_id = coalesce(excluded.collection_id, asset_items.collection_id),
                external_id = coalesce(excluded.external_id, asset_items.external_id),
                title = excluded.title,
                asset_kind = excluded.asset_kind,
                content_type = excluded.content_type,
                object_key = excluded.object_key,
                metadata = excluded.metadata,
                updated_at = now()
            returning id, tenant_id, asset_library_id, collection_id, external_id, title,
                      asset_kind, source_kind, source_id, content_type, object_key, metadata,
                      (
                          select count(*)
                          from asset_profiles p
                          where p.tenant_id = asset_items.tenant_id
                            and p.asset_id = asset_items.id
                      )::bigint as profile_count,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_asset.asset_library_id)
        .bind(new_asset.collection_id)
        .bind(new_asset.external_id)
        .bind(new_asset.title)
        .bind(new_asset.asset_kind)
        .bind(new_asset.source_kind)
        .bind(new_asset.source_id)
        .bind(new_asset.content_type)
        .bind(new_asset.object_key)
        .bind(new_asset.metadata)
        .fetch_one(&self.pool)
        .await?;

        map_asset_item_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
    ) -> Result<Option<AssetItemRecord>> {
        let row = sqlx::query(
            r#"
            select a.id,
                   a.tenant_id,
                   a.asset_library_id,
                   a.collection_id,
                   a.external_id,
                   a.title,
                   a.asset_kind,
                   a.source_kind,
                   a.source_id,
                   a.content_type,
                   a.object_key,
                   a.metadata,
                   (
                       select count(*)
                       from asset_profiles p
                       where p.tenant_id = a.tenant_id
                         and p.asset_id = a.id
                   )::bigint as profile_count,
                   a.created_at,
                   a.updated_at
            from asset_items a
            where a.tenant_id = $1 and a.id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_id)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_asset_item_row).transpose()
    }

    pub async fn list_by_asset_library(
        &self,
        tenant_id: TenantId,
        asset_library_id: Uuid,
        limit: usize,
    ) -> Result<Vec<AssetItemRecord>> {
        let rows = sqlx::query(
            r#"
            select a.id,
                   a.tenant_id,
                   a.asset_library_id,
                   a.collection_id,
                   a.external_id,
                   a.title,
                   a.asset_kind,
                   a.source_kind,
                   a.source_id,
                   a.content_type,
                   a.object_key,
                   a.metadata,
                   (
                       select count(*)
                       from asset_profiles p
                       where p.tenant_id = a.tenant_id
                         and p.asset_id = a.id
                   )::bigint as profile_count,
                   a.created_at,
                   a.updated_at
            from asset_items a
            where a.tenant_id = $1 and a.asset_library_id = $2
            order by a.created_at desc, a.title asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_library_id)
        .bind(limit.min(500) as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_asset_item_row).collect()
    }

    pub async fn list_by_dataset_ids(
        &self,
        tenant_id: TenantId,
        dataset_ids: &[DatasetId],
        limit: usize,
    ) -> Result<Vec<AssetItemRecord>> {
        if dataset_ids.is_empty() {
            return Ok(vec![]);
        }
        let dataset_ids = dataset_ids.iter().map(|id| id.0).collect::<Vec<_>>();
        let rows = sqlx::query(
            r#"
            select distinct on (a.id)
                   a.id,
                   a.tenant_id,
                   a.asset_library_id,
                   a.collection_id,
                   a.external_id,
                   a.title,
                   a.asset_kind,
                   a.source_kind,
                   a.source_id,
                   a.content_type,
                   a.object_key,
                   a.metadata,
                   (
                       select count(*)
                       from asset_profiles p
                       where p.tenant_id = a.tenant_id
                         and p.asset_id = a.id
                   )::bigint as profile_count,
                   a.created_at,
                   a.updated_at
            from asset_items a
            join dataset_asset_memberships m
              on m.tenant_id = a.tenant_id
             and m.asset_id = a.id
            where a.tenant_id = $1
              and m.dataset_id = any($2)
              and (m.expires_at is null or m.expires_at > now())
            order by a.id, a.created_at desc, a.title asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_ids)
        .bind(limit.min(500) as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_asset_item_row).collect()
    }

    pub async fn list_by_dataset_scope(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        limit: usize,
    ) -> Result<Vec<AssetItemRecord>> {
        let rows = sqlx::query(
            r#"
            select a.id, a.tenant_id, a.asset_library_id, a.collection_id,
                   a.external_id, a.title, a.asset_kind, a.source_kind, a.source_id,
                   a.content_type, a.object_key, a.metadata,
                   (
                       select count(*)
                       from asset_profiles p
                       where p.tenant_id = a.tenant_id and p.asset_id = a.id
                   )::bigint as profile_count,
                   a.created_at, a.updated_at
            from asset_items a
            join dataset_asset_memberships m
              on m.tenant_id = a.tenant_id and m.asset_id = a.id
            where a.tenant_id = $1
              and m.dataset_id = $2
              and (m.expires_at is null or m.expires_at > now())
            order by a.updated_at desc, a.id asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(limit.clamp(1, 10_000) as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_asset_item_row).collect()
    }

    pub async fn upsert_dataset_membership(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
        new_membership: NewDatasetAssetMembership,
    ) -> Result<DatasetAssetMembershipRecord> {
        let row = sqlx::query(
            r#"
            insert into dataset_asset_memberships (
                tenant_id,
                dataset_id,
                asset_id,
                membership_kind,
                expires_at
            )
            values ($1, $2, $3, $4, $5)
            on conflict (tenant_id, dataset_id, asset_id) do update
            set membership_kind = excluded.membership_kind,
                expires_at = excluded.expires_at
            returning tenant_id, dataset_id, asset_id, membership_kind, expires_at, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_membership.dataset_id.0)
        .bind(asset_id)
        .bind(new_membership.membership_kind)
        .bind(new_membership.expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(map_dataset_asset_membership_row(&row))
    }

    pub async fn list_dataset_memberships(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
    ) -> Result<Vec<DatasetAssetMembershipRecord>> {
        let rows = sqlx::query(
            r#"
            select tenant_id, dataset_id, asset_id, membership_kind, expires_at, created_at
            from dataset_asset_memberships
            where tenant_id = $1 and asset_id = $2
            order by created_at asc, dataset_id asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(map_dataset_asset_membership_row).collect())
    }

    pub async fn upsert_parse_run(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
        new_parse_run: NewAssetParseRun,
    ) -> Result<AssetParseRunRecord> {
        let row = sqlx::query(
            r#"
            insert into asset_parse_runs (
                tenant_id,
                asset_id,
                parser_name,
                parser_version,
                status,
                started_at,
                finished_at,
                error_code,
                error_message,
                metadata
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            on conflict (tenant_id, asset_id, parser_name, parser_version) do update
            set status = asset_parse_runs.status,
                started_at = asset_parse_runs.started_at,
                finished_at = asset_parse_runs.finished_at,
                error_code = asset_parse_runs.error_code,
                error_message = asset_parse_runs.error_message,
                metadata = asset_parse_runs.metadata || excluded.metadata,
                updated_at = now()
            returning id, tenant_id, asset_id, parser_name, parser_version, status,
                      started_at, finished_at, error_code, error_message, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_id)
        .bind(new_parse_run.parser_name)
        .bind(new_parse_run.parser_version)
        .bind(new_parse_run.status)
        .bind(new_parse_run.started_at)
        .bind(new_parse_run.finished_at)
        .bind(new_parse_run.error_code)
        .bind(new_parse_run.error_message)
        .bind(new_parse_run.metadata)
        .fetch_one(&self.pool)
        .await?;

        Ok(map_asset_parse_run_row(&row))
    }

    pub async fn get_parse_run_by_id(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
        parse_run_id: Uuid,
    ) -> Result<Option<AssetParseRunRecord>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, asset_id, parser_name, parser_version, status,
                   started_at, finished_at, error_code, error_message, metadata,
                   created_at, updated_at
            from asset_parse_runs
            where tenant_id = $1 and asset_id = $2 and id = $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_id)
        .bind(parse_run_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(map_asset_parse_run_row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_parse_run_state(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
        parse_run_id: Uuid,
        status: &str,
        started_at: Option<DateTime<Utc>>,
        finished_at: Option<DateTime<Utc>>,
        error_code: Option<&str>,
        error_message: Option<&str>,
        metadata_patch: &Value,
        updated_at: DateTime<Utc>,
    ) -> Result<AssetParseRunRecord> {
        let row = sqlx::query(
            r#"
            update asset_parse_runs
            set status = $4,
                started_at = coalesce($5, started_at),
                finished_at = $6,
                error_code = $7,
                error_message = $8,
                metadata = metadata || $9,
                updated_at = $10
            where tenant_id = $1 and asset_id = $2 and id = $3
            returning id, tenant_id, asset_id, parser_name, parser_version, status,
                      started_at, finished_at, error_code, error_message, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_id)
        .bind(parse_run_id)
        .bind(status)
        .bind(started_at)
        .bind(finished_at)
        .bind(error_code)
        .bind(error_message)
        .bind(metadata_patch)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(map_asset_parse_run_row(&row))
    }

    pub async fn list_parse_runs(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
    ) -> Result<Vec<AssetParseRunRecord>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, asset_id, parser_name, parser_version, status,
                   started_at, finished_at, error_code, error_message, metadata,
                   created_at, updated_at
            from asset_parse_runs
            where tenant_id = $1 and asset_id = $2
            order by updated_at desc, created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(map_asset_parse_run_row).collect())
    }

    pub async fn upsert_profile(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
        new_profile: NewAssetProfile,
    ) -> Result<AssetProfileRecord> {
        let row = sqlx::query(
            r#"
            insert into asset_profiles (
                tenant_id,
                asset_id,
                profile_kind,
                profile_version,
                attributes,
                embedding_status
            )
            values ($1, $2, $3, $4, $5, $6)
            on conflict (tenant_id, asset_id, profile_kind, profile_version) do update
            set attributes = excluded.attributes,
                embedding_status = excluded.embedding_status,
                updated_at = now()
            returning id, tenant_id, asset_id, profile_kind, profile_version, attributes,
                      embedding_status, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_id)
        .bind(new_profile.profile_kind)
        .bind(new_profile.profile_version)
        .bind(new_profile.attributes)
        .bind(new_profile.embedding_status)
        .fetch_one(&self.pool)
        .await?;

        Ok(map_asset_profile_row(&row))
    }

    pub async fn list_profiles(
        &self,
        tenant_id: TenantId,
        asset_id: Uuid,
    ) -> Result<Vec<AssetProfileRecord>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, asset_id, profile_kind, profile_version, attributes,
                   embedding_status, created_at, updated_at
            from asset_profiles
            where tenant_id = $1 and asset_id = $2
            order by profile_kind asc, profile_version desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(map_asset_profile_row).collect())
    }

    pub async fn list_profiles_by_asset_ids(
        &self,
        tenant_id: TenantId,
        asset_ids: &[Uuid],
        limit: usize,
    ) -> Result<Vec<AssetProfileRecord>> {
        if asset_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query(
            r#"
            select id, tenant_id, asset_id, profile_kind, profile_version,
                   attributes, embedding_status, created_at, updated_at
            from asset_profiles
            where tenant_id = $1
              and asset_id = any($2)
            order by asset_id asc, updated_at desc, profile_kind asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_ids)
        .bind(limit.max(asset_ids.len()).min(2000) as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(map_asset_profile_row).collect())
    }

    pub async fn list_accepted_profiles_by_asset_ids(
        &self,
        tenant_id: TenantId,
        asset_ids: &[Uuid],
        limit: usize,
    ) -> Result<Vec<AssetProfileRecord>> {
        if asset_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query(
            r#"
            select profile.id, profile.tenant_id, profile.asset_id, profile.profile_kind,
                   profile.profile_version, profile.attributes, profile.embedding_status,
                   profile.created_at, profile.updated_at
            from asset_profiles profile
            where profile.tenant_id = $1
              and profile.asset_id = any($2)
              and exists (
                  select 1
                  from asset_parse_runs parse_run
                  where parse_run.tenant_id = profile.tenant_id
                    and parse_run.asset_id = profile.asset_id
                    and parse_run.status in ('completed', 'partial')
                    and concat(parse_run.parser_name, '@', parse_run.parser_version)
                        = profile.profile_version
              )
            order by profile.asset_id asc, profile.updated_at desc,
                     profile.profile_kind asc, profile.profile_version desc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(asset_ids)
        .bind(limit.max(asset_ids.len()).min(2000) as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(map_asset_profile_row).collect())
    }

    pub async fn sync_document_asset_profile(
        &self,
        tenant_id: TenantId,
        document: &Document,
        chunks: &[DocumentChunk],
    ) -> Result<SyncedDocumentAssetProfile> {
        let asset = self
            .upsert_by_source(
                tenant_id,
                NewAssetItem {
                    asset_library_id: None,
                    collection_id: None,
                    external_id: Some(format!("document:{}", document.id.0)),
                    title: document.title.clone(),
                    asset_kind: document_asset_kind(document),
                    source_kind: "document".to_string(),
                    source_id: Some(document.id.0.to_string()),
                    content_type: Some(document.content_type.clone()),
                    object_key: Some(document.object_key.clone()),
                    metadata: document_asset_metadata(document),
                },
            )
            .await?;
        let dataset_membership = self
            .upsert_dataset_membership(
                tenant_id,
                asset.id,
                NewDatasetAssetMembership {
                    dataset_id: document.dataset_id,
                    membership_kind: "source_document".to_string(),
                    expires_at: None,
                },
            )
            .await?;
        let profile = self
            .upsert_profile(
                tenant_id,
                asset.id,
                NewAssetProfile {
                    profile_kind: document_asset_profile_kind(document),
                    profile_version: "v1".to_string(),
                    attributes: document_asset_profile_attributes(document, chunks),
                    embedding_status: "not_requested".to_string(),
                },
            )
            .await?;

        Ok(SyncedDocumentAssetProfile {
            asset,
            dataset_membership,
            profile,
        })
    }
}

#[derive(Clone)]
pub struct PgDocumentRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgDatasetDocumentMembershipRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgModelGatewayProfileRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgSecretBindingRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgUserRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgUserSessionRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgEmailVerificationChallengeRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgAuthAuditEventRepository {
    pool: PgPool,
}

impl PgSecretBindingRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_binding: NewSecretBinding,
    ) -> Result<SecretBinding> {
        let row = sqlx::query(
            r#"
            insert into secret_bindings (
                tenant_id,
                dataset_id,
                document_id,
                scope_level,
                provider_key,
                cipher_text,
                fingerprint
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            returning id, tenant_id, dataset_id, document_id, scope_level, provider_key, cipher_text, fingerprint, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_binding.dataset_id.0)
        .bind(new_binding.document_id.map(|id| id.0))
        .bind(new_binding.scope_level.as_str())
        .bind(new_binding.provider_key)
        .bind(new_binding.cipher_text)
        .bind(new_binding.fingerprint)
        .fetch_one(&self.pool)
        .await?;

        map_secret_binding_row(&row)
    }

    pub async fn list_by_fingerprint(
        &self,
        tenant_id: TenantId,
        fingerprint: &str,
    ) -> Result<Vec<SecretBinding>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, document_id, scope_level, provider_key, cipher_text, fingerprint, created_at
            from secret_bindings
            where tenant_id = $1 and fingerprint = $2
            order by created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(fingerprint)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_secret_binding_row).collect()
    }
}

impl PgUserRepository {
    pub async fn ensure_by_email(
        &self,
        tenant_id: TenantId,
        email: &str,
        display_name: Option<&str>,
    ) -> Result<User> {
        let email_normalized = normalize_email(email);
        let display_name = display_name
            .filter(|value| !value.trim().is_empty())
            .map(str::trim)
            .unwrap_or(&email_normalized);
        let row = sqlx::query(
            r#"
            insert into users (tenant_id, email, email_normalized, display_name, roles)
            values ($1, $2, $2, $3, '[]'::jsonb)
            on conflict (tenant_id, email_normalized) where email_normalized is not null do update
            set email_normalized = excluded.email_normalized,
                display_name = excluded.display_name
            returning id, tenant_id, email, display_name, roles, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(&email_normalized)
        .bind(display_name)
        .fetch_one(&self.pool)
        .await?;

        map_user_row(&row)
    }

    pub async fn get_by_email(&self, tenant_id: TenantId, email: &str) -> Result<Option<User>> {
        let email_normalized = normalize_email(email);
        let row = sqlx::query(
            r#"
            select id, tenant_id, email, display_name, roles, created_at
            from users
            where tenant_id = $1 and email_normalized = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(email_normalized)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_user_row).transpose()
    }

    pub async fn get_by_id(&self, tenant_id: TenantId, user_id: UserId) -> Result<Option<User>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, email, display_name, roles, created_at
            from users
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(user_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_user_row).transpose()
    }

    pub async fn update_primary_secret_fingerprint(
        &self,
        tenant_id: TenantId,
        user_id: UserId,
        fingerprint: &str,
        logged_in_at: DateTime<Utc>,
    ) -> Result<Option<User>> {
        let row = sqlx::query(
            r#"
            update users
            set primary_secret_fingerprint = $3,
                last_login_at = $4
            where tenant_id = $1 and id = $2
            returning id, tenant_id, email, display_name, roles, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(user_id.0)
        .bind(fingerprint)
        .bind(logged_in_at)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_user_row).transpose()
    }
}

impl PgUserSessionRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_session: NewUserSession,
    ) -> Result<UserSession> {
        let row = sqlx::query(
            r#"
            insert into user_sessions (
                tenant_id,
                user_id,
                device_fingerprint,
                session_token_hash,
                auth_method,
                created_at,
                last_seen_at,
                expires_at
            )
            values ($1, $2, $3, $4, $5, $6, $6, $7)
            returning id, tenant_id, user_id, device_fingerprint, session_token_hash, auth_method,
                      created_at, last_seen_at, expires_at, revoked_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_session.user_id.0)
        .bind(new_session.device_fingerprint)
        .bind(new_session.session_token_hash)
        .bind(new_session.auth_method.as_str())
        .bind(new_session.created_at)
        .bind(new_session.expires_at)
        .fetch_one(&self.pool)
        .await?;

        map_user_session_row(&row)
    }

    pub async fn get_by_token_hash(
        &self,
        tenant_id: TenantId,
        token_hash: &str,
    ) -> Result<Option<UserSession>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, user_id, device_fingerprint, session_token_hash, auth_method,
                   created_at, last_seen_at, expires_at, revoked_at
            from user_sessions
            where tenant_id = $1 and session_token_hash = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_user_session_row).transpose()
    }

    pub async fn revoke(
        &self,
        tenant_id: TenantId,
        session_id: UserSessionId,
        revoked_at: DateTime<Utc>,
    ) -> Result<Option<UserSession>> {
        let row = sqlx::query(
            r#"
            update user_sessions
            set revoked_at = $3
            where tenant_id = $1 and id = $2
            returning id, tenant_id, user_id, device_fingerprint, session_token_hash, auth_method,
                      created_at, last_seen_at, expires_at, revoked_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(session_id.0)
        .bind(revoked_at)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_user_session_row).transpose()
    }
}

impl PgEmailVerificationChallengeRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_challenge: NewEmailVerificationChallenge,
    ) -> Result<EmailVerificationChallenge> {
        let row = sqlx::query(
            r#"
            insert into email_verification_challenges (
                tenant_id,
                email_normalized,
                purpose,
                code_hash,
                max_attempts,
                expires_at,
                metadata,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            returning id, tenant_id, email_normalized, purpose, code_hash, attempt_count,
                      max_attempts, expires_at, consumed_at, metadata, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_challenge.email_normalized)
        .bind(new_challenge.purpose.as_str())
        .bind(new_challenge.code_hash)
        .bind(new_challenge.max_attempts)
        .bind(new_challenge.expires_at)
        .bind(new_challenge.metadata)
        .bind(new_challenge.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_email_verification_challenge_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        challenge_id: EmailVerificationChallengeId,
    ) -> Result<Option<EmailVerificationChallenge>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, email_normalized, purpose, code_hash, attempt_count,
                   max_attempts, expires_at, consumed_at, metadata, created_at
            from email_verification_challenges
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(challenge_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref()
            .map(map_email_verification_challenge_row)
            .transpose()
    }

    pub async fn latest_active_by_email_and_purpose(
        &self,
        tenant_id: TenantId,
        email_normalized: &str,
        purpose: AuthChallengePurpose,
    ) -> Result<Option<EmailVerificationChallenge>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, email_normalized, purpose, code_hash, attempt_count,
                   max_attempts, expires_at, consumed_at, metadata, created_at
            from email_verification_challenges
            where tenant_id = $1
              and email_normalized = $2
              and purpose = $3
              and consumed_at is null
            order by created_at desc
            limit 1
            "#,
        )
        .bind(tenant_id.0)
        .bind(email_normalized)
        .bind(purpose.as_str())
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref()
            .map(map_email_verification_challenge_row)
            .transpose()
    }

    pub async fn count_created_since_by_email_and_purpose(
        &self,
        tenant_id: TenantId,
        email_normalized: &str,
        purpose: AuthChallengePurpose,
        since: DateTime<Utc>,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            select count(*) as challenge_count
            from email_verification_challenges
            where tenant_id = $1
              and email_normalized = $2
              and purpose = $3
              and created_at >= $4
            "#,
        )
        .bind(tenant_id.0)
        .bind(email_normalized)
        .bind(purpose.as_str())
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64, _>("challenge_count"))
    }

    pub async fn count_created_since_by_device_and_purpose(
        &self,
        tenant_id: TenantId,
        device_fingerprint: &str,
        purpose: AuthChallengePurpose,
        since: DateTime<Utc>,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            select count(*) as challenge_count
            from email_verification_challenges
            where tenant_id = $1
              and metadata ->> 'device_fingerprint' = $2
              and purpose = $3
              and created_at >= $4
            "#,
        )
        .bind(tenant_id.0)
        .bind(device_fingerprint)
        .bind(purpose.as_str())
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64, _>("challenge_count"))
    }

    pub async fn increment_attempt_count(
        &self,
        tenant_id: TenantId,
        challenge_id: EmailVerificationChallengeId,
    ) -> Result<Option<EmailVerificationChallenge>> {
        let row = sqlx::query(
            r#"
            update email_verification_challenges
            set attempt_count = attempt_count + 1
            where tenant_id = $1 and id = $2
            returning id, tenant_id, email_normalized, purpose, code_hash, attempt_count,
                      max_attempts, expires_at, consumed_at, metadata, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(challenge_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref()
            .map(map_email_verification_challenge_row)
            .transpose()
    }

    pub async fn mark_consumed(
        &self,
        tenant_id: TenantId,
        challenge_id: EmailVerificationChallengeId,
        consumed_at: DateTime<Utc>,
    ) -> Result<Option<EmailVerificationChallenge>> {
        let row = sqlx::query(
            r#"
            update email_verification_challenges
            set consumed_at = $3
            where tenant_id = $1 and id = $2
            returning id, tenant_id, email_normalized, purpose, code_hash, attempt_count,
                      max_attempts, expires_at, consumed_at, metadata, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(challenge_id.0)
        .bind(consumed_at)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref()
            .map(map_email_verification_challenge_row)
            .transpose()
    }
}

impl PgDocumentRepository {
    pub async fn create(&self, tenant_id: TenantId, new_document: NewDocument) -> Result<Document> {
        let row = sqlx::query(
            r#"
            insert into documents (
                tenant_id,
                dataset_id,
                owner_user_id,
                title,
                object_key,
                content_type,
                lifecycle,
                metadata
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            returning id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_document.dataset_id.0)
        .bind(new_document.owner_user_id.map(|id| id.0))
        .bind(new_document.title)
        .bind(new_document.object_key)
        .bind(new_document.content_type)
        .bind(DocumentLifecycle::Received.as_str())
        .bind(document_initial_metadata(
            &new_document.secret_binding_ids,
            &new_document.metadata,
        )?)
        .fetch_one(&self.pool)
        .await?;

        map_document_row(&row)
    }

    pub async fn list_by_tenant(&self, tenant_id: TenantId) -> Result<Vec<Document>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            from documents
            where tenant_id = $1
            order by created_at desc, title asc
            "#,
        )
        .bind(tenant_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_document_row).collect()
    }

    pub async fn list_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Vec<Document>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            from documents
            where tenant_id = $1 and dataset_id = $2
            order by created_at desc, title asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_document_row).collect()
    }

    pub async fn list_by_dataset_scope(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Vec<Document>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            from documents
            where tenant_id = $1 and dataset_id = $2
            union
            select d.id, d.tenant_id, d.dataset_id, d.owner_user_id, d.title, d.object_key, d.content_type, d.lifecycle, d.metadata, d.created_at, d.updated_at
            from documents d
            join dataset_document_memberships m
              on m.tenant_id = d.tenant_id
             and m.document_id = d.id
            where m.tenant_id = $1
              and m.dataset_id = $2
              and (m.expires_at is null or m.expires_at > now())
            order by created_at desc, title asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_document_row).collect()
    }

    pub async fn list_by_dataset_scope_bounded(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        as_of: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<Document>> {
        let rows = sqlx::query(DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL)
            .bind(tenant_id.0)
            .bind(dataset_id.0)
            .bind(as_of)
            .bind(limit.clamp(1, 10_000) as i64)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(map_document_row).collect()
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
    ) -> Result<Option<Document>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            from documents
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_document_row).transpose()
    }

    pub async fn update_state(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
        lifecycle: DocumentLifecycle,
        title: Option<&str>,
        metadata_updates: &Value,
        updated_at: DateTime<Utc>,
    ) -> Result<Document> {
        let current = self
            .get_by_id(tenant_id, document_id)
            .await?
            .ok_or_else(|| anyhow!("document {document_id} not found for tenant {tenant_id}"))?;
        let merged_metadata = merge_document_metadata(&current.metadata, metadata_updates)?;
        let next_title = title.unwrap_or(&current.title).to_string();

        let row = sqlx::query(
            r#"
            update documents
            set title = $3,
                lifecycle = $4,
                metadata = $5,
                updated_at = $6
            where tenant_id = $1 and id = $2
            returning id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .bind(next_title)
        .bind(lifecycle.as_str())
        .bind(merged_metadata)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_document_row(&row)
    }

    pub async fn move_to_dataset(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
        dataset_id: DatasetId,
        metadata_updates: &Value,
        updated_at: DateTime<Utc>,
    ) -> Result<Document> {
        let current = self
            .get_by_id(tenant_id, document_id)
            .await?
            .ok_or_else(|| anyhow!("document {document_id} not found for tenant {tenant_id}"))?;
        let merged_metadata = merge_document_metadata(&current.metadata, metadata_updates)?;

        let row = sqlx::query(
            r#"
            update documents
            set dataset_id = $3,
                metadata = $4,
                updated_at = $5
            where tenant_id = $1 and id = $2
            returning id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .bind(dataset_id.0)
        .bind(merged_metadata)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_document_row(&row)
    }

    pub async fn update_owner_user_id(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
        owner_user_id: UserId,
    ) -> Result<Option<Document>> {
        let row = sqlx::query(
            r#"
            update documents
            set owner_user_id = $3,
                updated_at = now()
            where tenant_id = $1
              and id = $2
              and (owner_user_id is null or owner_user_id = $3)
            returning id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .bind(owner_user_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_document_row).transpose()
    }

    pub async fn record_content_fingerprint(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
        content_sha256: &str,
        content_size_bytes: i64,
        recorded_at: DateTime<Utc>,
    ) -> Result<Document> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            insert into document_content_fingerprints (
                tenant_id,
                content_sha256,
                content_size_bytes,
                canonical_document_id,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $5)
            on conflict (tenant_id, content_sha256) do nothing
            "#,
        )
        .bind(tenant_id.0)
        .bind(content_sha256)
        .bind(content_size_bytes)
        .bind(document_id.0)
        .bind(recorded_at)
        .execute(&mut *tx)
        .await?;

        let canonical_document_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            select canonical_document_id
            from document_content_fingerprints
            where tenant_id = $1 and content_sha256 = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(content_sha256)
        .fetch_one(&mut *tx)
        .await?;
        let dedup_state = if canonical_document_id == document_id.0 {
            "canonical"
        } else {
            "duplicate"
        };

        let row = sqlx::query(
            r#"
            update documents
            set content_sha256 = $3,
                content_size_bytes = $4,
                canonical_document_id = $5,
                dedup_state = $6,
                deduped_at = $7,
                updated_at = $7
            where tenant_id = $1 and id = $2
            returning id, tenant_id, dataset_id, owner_user_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .bind(content_sha256)
        .bind(content_size_bytes)
        .bind(canonical_document_id)
        .bind(dedup_state)
        .bind(recorded_at)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;

        map_document_row(&row)
    }
}

pub const DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL: &str = r#"
    select distinct on (scoped.id)
           scoped.id, scoped.tenant_id, scoped.dataset_id, scoped.owner_user_id,
           scoped.title, scoped.object_key, scoped.content_type, scoped.lifecycle,
           scoped.metadata, scoped.created_at, scoped.updated_at
    from (
        select d.id, d.tenant_id, d.dataset_id, d.owner_user_id, d.title,
               d.object_key, d.content_type, d.lifecycle, d.metadata,
               d.created_at, d.updated_at
        from documents d
        where d.tenant_id = $1
          and d.dataset_id = $2
          and d.created_at <= $3
        union all
        select d.id, d.tenant_id, d.dataset_id, d.owner_user_id, d.title,
               d.object_key, d.content_type, d.lifecycle, d.metadata,
               d.created_at, d.updated_at
        from dataset_document_memberships m
        join documents d
          on d.tenant_id = m.tenant_id
         and d.id = m.document_id
        where m.tenant_id = $1
          and m.dataset_id = $2
          and m.created_at <= $3
          and d.created_at <= $3
          and (m.expires_at is null or m.expires_at > $3)
    ) scoped
    order by scoped.id, scoped.updated_at desc
    limit $4
"#;

async fn resolve_canonical_document_id(
    pool: &PgPool,
    tenant_id: TenantId,
    document_id: DocumentId,
) -> Result<DocumentId> {
    let resolved = sqlx::query_scalar::<_, Uuid>(
        r#"
        select coalesce(canonical_document_id, id) as document_id
        from documents
        where tenant_id = $1 and id = $2
        "#,
    )
    .bind(tenant_id.0)
    .bind(document_id.0)
    .fetch_optional(pool)
    .await?;

    Ok(DocumentId(resolved.unwrap_or(document_id.0)))
}

#[derive(Clone)]
pub struct PgDocumentEnrichmentRunRepository {
    pool: PgPool,
}

impl PgDocumentEnrichmentRunRepository {
    pub async fn create_or_get(
        &self,
        tenant_id: TenantId,
        new_run: &NewDocumentEnrichmentRun,
        created_at: DateTime<Utc>,
    ) -> Result<DocumentEnrichmentRun> {
        let row = sqlx::query(
            r#"
            insert into document_enrichment_runs (
                tenant_id,
                document_id,
                enrichment_kind,
                parse_version,
                input_fingerprint,
                priority,
                max_attempts,
                available_at,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            on conflict (tenant_id, document_id, enrichment_kind, input_fingerprint)
            do update set
                priority = least(document_enrichment_runs.priority, excluded.priority),
                max_attempts = greatest(document_enrichment_runs.max_attempts, excluded.max_attempts),
                available_at = least(document_enrichment_runs.available_at, excluded.available_at),
                parse_version = coalesce(excluded.parse_version, document_enrichment_runs.parse_version),
                updated_at = excluded.updated_at
            returning id, tenant_id, document_id, enrichment_kind, parse_version, input_fingerprint,
                      status, priority, attempt_count, max_attempts, available_at, started_at,
                      finished_at, error_message, output_summary, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_run.document_id.0)
        .bind(&new_run.enrichment_kind)
        .bind(&new_run.parse_version)
        .bind(&new_run.input_fingerprint)
        .bind(new_run.priority)
        .bind(new_run.max_attempts)
        .bind(new_run.available_at)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;

        map_document_enrichment_run_row(&row)
    }

    pub async fn claim_next_available(
        &self,
        tenant_id: TenantId,
        enrichment_kind: Option<&str>,
        claimed_at: DateTime<Utc>,
    ) -> Result<Option<DocumentEnrichmentRun>> {
        let row = sqlx::query(
            r#"
            with next_run as (
                select id
                from document_enrichment_runs
                where tenant_id = $1
                  and status = 'pending'
                  and available_at <= $2
                  and attempt_count < max_attempts
                  and ($3::text is null or enrichment_kind = $3)
                order by priority asc, available_at asc, created_at asc, id asc
                for update skip locked
                limit 1
            )
            update document_enrichment_runs
            set status = 'running',
                attempt_count = attempt_count + 1,
                started_at = $2,
                error_message = null,
                updated_at = $2
            where id in (select id from next_run)
            returning id, tenant_id, document_id, enrichment_kind, parse_version, input_fingerprint,
                      status, priority, attempt_count, max_attempts, available_at, started_at,
                      finished_at, error_message, output_summary, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(claimed_at)
        .bind(enrichment_kind)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref()
            .map(map_document_enrichment_run_row)
            .transpose()
    }

    pub async fn mark_succeeded(
        &self,
        tenant_id: TenantId,
        run_id: Uuid,
        output_summary: &Value,
        finished_at: DateTime<Utc>,
    ) -> Result<DocumentEnrichmentRun> {
        let row = sqlx::query(
            r#"
            update document_enrichment_runs
            set status = 'succeeded',
                finished_at = $3,
                error_message = null,
                output_summary = $4,
                updated_at = $3
            where tenant_id = $1 and id = $2
            returning id, tenant_id, document_id, enrichment_kind, parse_version, input_fingerprint,
                      status, priority, attempt_count, max_attempts, available_at, started_at,
                      finished_at, error_message, output_summary, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(run_id)
        .bind(finished_at)
        .bind(output_summary)
        .fetch_one(&self.pool)
        .await?;

        map_document_enrichment_run_row(&row)
    }

    pub async fn requeue_after_error(
        &self,
        tenant_id: TenantId,
        run_id: Uuid,
        error_message: &str,
        available_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<DocumentEnrichmentRun> {
        let row = sqlx::query(
            r#"
            update document_enrichment_runs
            set status = case
                    when attempt_count >= max_attempts then 'failed'
                    else 'pending'
                end,
                available_at = $3,
                finished_at = case
                    when attempt_count >= max_attempts then $4
                    else null
                end,
                error_message = $5,
                updated_at = $4
            where tenant_id = $1 and id = $2
            returning id, tenant_id, document_id, enrichment_kind, parse_version, input_fingerprint,
                      status, priority, attempt_count, max_attempts, available_at, started_at,
                      finished_at, error_message, output_summary, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(run_id)
        .bind(available_at)
        .bind(updated_at)
        .bind(error_message)
        .fetch_one(&self.pool)
        .await?;

        map_document_enrichment_run_row(&row)
    }

    pub async fn mark_failed(
        &self,
        tenant_id: TenantId,
        run_id: Uuid,
        error_message: &str,
        finished_at: DateTime<Utc>,
    ) -> Result<DocumentEnrichmentRun> {
        let row = sqlx::query(
            r#"
            update document_enrichment_runs
            set status = 'failed',
                finished_at = $3,
                error_message = $4,
                updated_at = $3
            where tenant_id = $1 and id = $2
            returning id, tenant_id, document_id, enrichment_kind, parse_version, input_fingerprint,
                      status, priority, attempt_count, max_attempts, available_at, started_at,
                      finished_at, error_message, output_summary, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(run_id)
        .bind(finished_at)
        .bind(error_message)
        .fetch_one(&self.pool)
        .await?;

        map_document_enrichment_run_row(&row)
    }

    pub async fn list_by_document(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
        limit: i64,
    ) -> Result<Vec<DocumentEnrichmentRun>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, document_id, enrichment_kind, parse_version, input_fingerprint,
                   status, priority, attempt_count, max_attempts, available_at, started_at,
                   finished_at, error_message, output_summary, created_at, updated_at
            from document_enrichment_runs
            where tenant_id = $1 and document_id = $2
            order by created_at desc, id asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_document_enrichment_run_row).collect()
    }
}

impl PgDatasetDocumentMembershipRepository {
    pub async fn create_or_update(
        &self,
        tenant_id: TenantId,
        new_membership: NewDatasetDocumentMembership,
    ) -> Result<DatasetDocumentMembership> {
        let row = sqlx::query(
            r#"
            insert into dataset_document_memberships (
                tenant_id,
                dataset_id,
                document_id,
                membership_kind,
                source,
                expires_at
            )
            values ($1, $2, $3, $4, $5, $6)
            on conflict (tenant_id, dataset_id, document_id) do update
            set membership_kind = excluded.membership_kind,
                source = excluded.source,
                expires_at = excluded.expires_at
            returning tenant_id, dataset_id, document_id, membership_kind, source, expires_at, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_membership.dataset_id.0)
        .bind(new_membership.document_id.0)
        .bind(new_membership.membership_kind)
        .bind(new_membership.source)
        .bind(new_membership.expires_at)
        .fetch_one(&self.pool)
        .await?;

        map_dataset_document_membership_row(&row)
    }

    pub async fn list_document_ids_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Vec<DocumentId>> {
        let rows = sqlx::query(
            r#"
            select document_id
            from dataset_document_memberships
            where tenant_id = $1
              and dataset_id = $2
              and (expires_at is null or expires_at > now())
            order by document_id
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| DocumentId(row.get::<Uuid, _>("document_id")))
            .collect())
    }

    pub async fn list_active_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        as_of: DateTime<Utc>,
    ) -> Result<Vec<DatasetDocumentMembership>> {
        let rows = sqlx::query(DATASET_SEMANTIC_ACTIVE_MEMBERSHIPS_SQL)
            .bind(tenant_id.0)
            .bind(dataset_id.0)
            .bind(as_of)
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(map_dataset_document_membership_row)
            .collect()
    }

    pub async fn list_dataset_ids_by_document(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
    ) -> Result<Vec<DatasetId>> {
        let rows = sqlx::query(
            r#"
            select dataset_id
            from dataset_document_memberships
            where tenant_id = $1
              and document_id = $2
              and (expires_at is null or expires_at > now())
            order by dataset_id
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| DatasetId(row.get::<Uuid, _>("dataset_id")))
            .collect())
    }

    pub async fn list_dataset_ids_by_documents(
        &self,
        tenant_id: TenantId,
        document_ids: &[DocumentId],
    ) -> Result<Vec<(DocumentId, DatasetId)>> {
        if document_ids.is_empty() {
            return Ok(Vec::new());
        }

        let ids: Vec<Uuid> = document_ids.iter().map(|id| id.0).collect();
        let rows = sqlx::query(
            r#"
            select document_id, dataset_id
            from dataset_document_memberships
            where tenant_id = $1
              and document_id = any($2)
              and (expires_at is null or expires_at > now())
            order by document_id, dataset_id
            "#,
        )
        .bind(tenant_id.0)
        .bind(ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                (
                    DocumentId(row.get::<Uuid, _>("document_id")),
                    DatasetId(row.get::<Uuid, _>("dataset_id")),
                )
            })
            .collect())
    }

    pub async fn delete(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        document_id: DocumentId,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            delete from dataset_document_memberships
            where tenant_id = $1
              and dataset_id = $2
              and document_id = $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(document_id.0)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn delete_expired(&self, tenant_id: TenantId, now: DateTime<Utc>) -> Result<u64> {
        let result = sqlx::query(
            r#"
            delete from dataset_document_memberships
            where tenant_id = $1
              and expires_at is not null
              and expires_at <= $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

pub const DATASET_SEMANTIC_ACTIVE_MEMBERSHIPS_SQL: &str = r#"
    select tenant_id, dataset_id, document_id, membership_kind, source, expires_at, created_at
    from dataset_document_memberships
    where tenant_id = $1
      and dataset_id = $2
      and created_at <= $3
      and (expires_at is null or expires_at > $3)
    order by document_id
"#;

impl PgModelGatewayProfileRepository {
    pub async fn list_enabled_by_lane(
        &self,
        tenant_id: TenantId,
        lane: &str,
    ) -> Result<Vec<ModelGatewayProfile>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, profile_id, display_name, lane, provider_id, model_id,
                   base_url, api_path, wire_api, auth_mode, auth_env_key_name,
                   recommended_preset, max_concurrency, rpm_limit, tpm_limit, timeout_ms,
                   priority, enabled, capabilities, created_at, updated_at
            from model_gateway_profiles
            where tenant_id = $1 and lane = $2 and enabled = true
            order by priority desc, profile_id asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(lane)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_model_gateway_profile_row).collect()
    }

    pub async fn list_all(&self, tenant_id: TenantId) -> Result<Vec<ModelGatewayProfile>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, profile_id, display_name, lane, provider_id, model_id,
                   base_url, api_path, wire_api, auth_mode, auth_env_key_name,
                   recommended_preset, max_concurrency, rpm_limit, tpm_limit, timeout_ms,
                   priority, enabled, capabilities, created_at, updated_at
            from model_gateway_profiles
            where tenant_id = $1
            order by lane asc, priority desc, profile_id asc
            "#,
        )
        .bind(tenant_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_model_gateway_profile_row).collect()
    }

    pub async fn get_by_profile_id(
        &self,
        tenant_id: TenantId,
        profile_id: &str,
    ) -> Result<Option<ModelGatewayProfile>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, profile_id, display_name, lane, provider_id, model_id,
                   base_url, api_path, wire_api, auth_mode, auth_env_key_name,
                   recommended_preset, max_concurrency, rpm_limit, tpm_limit, timeout_ms,
                   priority, enabled, capabilities, created_at, updated_at
            from model_gateway_profiles
            where tenant_id = $1 and profile_id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_model_gateway_profile_row).transpose()
    }

    pub async fn create(
        &self,
        tenant_id: TenantId,
        profile: NewModelGatewayProfile,
    ) -> Result<ModelGatewayProfile> {
        let row = sqlx::query(
            r#"
            insert into model_gateway_profiles (
                id, tenant_id, profile_id, display_name, lane, provider_id, model_id,
                base_url, api_path, wire_api, auth_mode, auth_env_key_name,
                recommended_preset, max_concurrency, rpm_limit, tpm_limit, timeout_ms,
                priority, enabled, capabilities
            )
            values (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17,
                $18, $19, $20
            )
            returning id, tenant_id, profile_id, display_name, lane, provider_id, model_id,
                      base_url, api_path, wire_api, auth_mode, auth_env_key_name,
                      recommended_preset, max_concurrency, rpm_limit, tpm_limit, timeout_ms,
                      priority, enabled, capabilities, created_at, updated_at
            "#,
        )
        .bind(profile.id)
        .bind(tenant_id.0)
        .bind(profile.profile_id)
        .bind(profile.display_name)
        .bind(profile.lane)
        .bind(profile.provider_id)
        .bind(profile.model_id)
        .bind(profile.base_url)
        .bind(profile.api_path)
        .bind(profile.wire_api)
        .bind(profile.auth_mode)
        .bind(profile.auth_env_key_name)
        .bind(profile.recommended_preset)
        .bind(profile.max_concurrency)
        .bind(profile.rpm_limit)
        .bind(profile.tpm_limit)
        .bind(profile.timeout_ms)
        .bind(profile.priority)
        .bind(profile.enabled)
        .bind(profile.capabilities)
        .fetch_one(&self.pool)
        .await?;

        map_model_gateway_profile_row(&row)
    }

    pub async fn update(
        &self,
        tenant_id: TenantId,
        profile_id: &str,
        update: ModelGatewayProfileUpdate,
    ) -> Result<Option<ModelGatewayProfile>> {
        let row = sqlx::query(
            r#"
            update model_gateway_profiles
            set display_name = coalesce($3, display_name),
                lane = coalesce($4, lane),
                provider_id = coalesce($5, provider_id),
                model_id = coalesce($6, model_id),
                base_url = case when $7::text is null then base_url else nullif($7, '') end,
                api_path = case when $8::text is null then api_path else nullif($8, '') end,
                wire_api = coalesce($9, wire_api),
                auth_mode = coalesce($10, auth_mode),
                auth_env_key_name = case when $11::text is null then auth_env_key_name else nullif($11, '') end,
                recommended_preset = case when $12::text is null then recommended_preset else nullif($12, '') end,
                max_concurrency = coalesce($13, max_concurrency),
                rpm_limit = coalesce($14, rpm_limit),
                tpm_limit = coalesce($15, tpm_limit),
                timeout_ms = coalesce($16, timeout_ms),
                priority = coalesce($17, priority),
                enabled = coalesce($18, enabled),
                capabilities = coalesce($19, capabilities),
                updated_at = now()
            where tenant_id = $1 and profile_id = $2
            returning id, tenant_id, profile_id, display_name, lane, provider_id, model_id,
                      base_url, api_path, wire_api, auth_mode, auth_env_key_name,
                      recommended_preset, max_concurrency, rpm_limit, tpm_limit, timeout_ms,
                      priority, enabled, capabilities, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(profile_id)
        .bind(update.display_name)
        .bind(update.lane)
        .bind(update.provider_id)
        .bind(update.model_id)
        .bind(update.base_url)
        .bind(update.api_path)
        .bind(update.wire_api)
        .bind(update.auth_mode)
        .bind(update.auth_env_key_name)
        .bind(update.recommended_preset)
        .bind(update.max_concurrency)
        .bind(update.rpm_limit)
        .bind(update.tpm_limit)
        .bind(update.timeout_ms)
        .bind(update.priority)
        .bind(update.enabled)
        .bind(update.capabilities)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_model_gateway_profile_row).transpose()
    }

    pub async fn disable(
        &self,
        tenant_id: TenantId,
        profile_id: &str,
    ) -> Result<Option<ModelGatewayProfile>> {
        self.update(
            tenant_id,
            profile_id,
            ModelGatewayProfileUpdate {
                enabled: Some(false),
                ..ModelGatewayProfileUpdate::default()
            },
        )
        .await
    }

    pub async fn record_event(
        &self,
        tenant_id: TenantId,
        event: NewModelGatewayProfileEvent,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into model_gateway_profile_events (
                id, tenant_id, profile_id, lane, event_type, latency_ms,
                input_tokens, output_tokens, error_kind, created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id.0)
        .bind(event.profile_id)
        .bind(event.lane)
        .bind(event.event_type)
        .bind(event.latency_ms)
        .bind(event.input_tokens)
        .bind(event.output_tokens)
        .bind(event.error_kind)
        .bind(event.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn summarize_recent_usage(
        &self,
        tenant_id: TenantId,
        since: DateTime<Utc>,
    ) -> Result<Vec<ModelGatewayProfileUsageSummary>> {
        let rows = sqlx::query(
            r#"
            select profile_id,
                   lane,
                   count(*) filter (where event_type in ('success', 'failure', 'timeout', 'rate_limit'))::bigint as request_count,
                   count(*) filter (where event_type = 'success')::bigint as success_count,
                   count(*) filter (where event_type in ('failure', 'timeout', 'rate_limit'))::bigint as failure_count,
                   count(*) filter (where event_type = 'timeout')::bigint as timeout_count,
                   count(*) filter (where event_type = 'rate_limit')::bigint as rate_limit_count,
                   count(*) filter (where event_type = 'would_throttle')::bigint as would_throttle_count,
                   coalesce(sum(input_tokens), 0)::bigint as input_tokens,
                   coalesce(sum(output_tokens), 0)::bigint as output_tokens,
                   count(*) filter (where event_type in ('shadow_quality_pass', 'shadow_quality_fail'))::bigint as shadow_eval_count,
                   count(*) filter (where event_type = 'shadow_quality_pass')::bigint as shadow_eval_pass_count,
                   count(*) filter (where event_type = 'shadow_quality_fail')::bigint as shadow_eval_fail_count,
                   count(*) filter (where event_type = 'shadow_quality_pass')::bigint as shadow_eval_format_pass_count,
                   count(*) filter (where error_kind = 'repair')::bigint as shadow_eval_repair_count,
                   max(created_at) filter (where event_type in ('shadow_quality_pass', 'shadow_quality_fail')) as last_shadow_eval_at,
                   (
                     array_agg(
                       case event_type
                         when 'profile_test_ok' then 'ok'
                         when 'profile_test_failed' then 'failed'
                         when 'profile_test_missing_secret' then 'missing_secret'
                         else null
                       end
                       order by created_at desc
                     ) filter (where event_type in ('profile_test_ok', 'profile_test_failed', 'profile_test_missing_secret'))
                   )[1] as last_profile_test_status,
                   max(created_at) filter (where event_type in ('profile_test_ok', 'profile_test_failed', 'profile_test_missing_secret')) as last_profile_test_at
            from model_gateway_profile_events
            where tenant_id = $1 and created_at >= $2
            group by profile_id, lane
            order by lane asc, profile_id asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| ModelGatewayProfileUsageSummary {
                profile_id: row.get("profile_id"),
                lane: row.get("lane"),
                request_count: row.get("request_count"),
                success_count: row.get("success_count"),
                failure_count: row.get("failure_count"),
                timeout_count: row.get("timeout_count"),
                rate_limit_count: row.get("rate_limit_count"),
                would_throttle_count: row.get("would_throttle_count"),
                input_tokens: row.get("input_tokens"),
                output_tokens: row.get("output_tokens"),
                shadow_eval_count: row.get("shadow_eval_count"),
                shadow_eval_pass_count: row.get("shadow_eval_pass_count"),
                shadow_eval_fail_count: row.get("shadow_eval_fail_count"),
                shadow_eval_format_pass_count: row.get("shadow_eval_format_pass_count"),
                shadow_eval_repair_count: row.get("shadow_eval_repair_count"),
                last_shadow_eval_at: row.get("last_shadow_eval_at"),
                last_profile_test_status: row.get("last_profile_test_status"),
                last_profile_test_at: row.get("last_profile_test_at"),
            })
            .collect())
    }
}

#[derive(Clone)]
pub struct PgDocumentChunkRepository {
    pool: PgPool,
}

impl PgDocumentChunkRepository {
    pub async fn replace_for_document(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
        chunks: &[NewDocumentChunk],
    ) -> Result<Vec<DocumentChunk>> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            delete from document_chunks
            where tenant_id = $1 and document_id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .execute(&mut *tx)
        .await?;

        let mut persisted = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let row = sqlx::query(
                r#"
                insert into document_chunks (
                    id,
                    tenant_id,
                    dataset_id,
                    document_id,
                    chunk_index,
                    content,
                    token_count,
                    state,
                    metadata,
                    created_at,
                    updated_at
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
                returning id, tenant_id, dataset_id, document_id, chunk_index, content, token_count, state, metadata, created_at, updated_at
                "#,
            )
            .bind(DocumentChunkId::new().0)
            .bind(tenant_id.0)
            .bind(chunk.dataset_id.0)
            .bind(chunk.document_id.0)
            .bind(chunk.chunk_index)
            .bind(&chunk.content)
            .bind(chunk.token_count)
            .bind(DocumentChunkState::Extracted.as_str())
            .bind(&chunk.metadata)
            .bind(chunk.created_at)
            .fetch_one(&mut *tx)
            .await?;

            persisted.push(map_document_chunk_row(&row)?);
        }

        tx.commit().await?;
        Ok(persisted)
    }

    pub async fn list_by_document(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
    ) -> Result<Vec<DocumentChunk>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, document_id, chunk_index, content, token_count, state, metadata, created_at, updated_at
            from document_chunks
            where tenant_id = $1 and document_id = $2
            order by chunk_index asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_document_chunk_row).collect()
    }

    pub async fn list_by_document_or_canonical(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
    ) -> Result<Vec<DocumentChunk>> {
        let effective_document_id =
            resolve_canonical_document_id(&self.pool, tenant_id, document_id).await?;
        self.list_by_document(tenant_id, effective_document_id)
            .await
    }

    pub async fn list_by_documents_bounded(
        &self,
        tenant_id: TenantId,
        document_ids: &[DocumentId],
        limit: usize,
    ) -> Result<Vec<DocumentChunk>> {
        if document_ids.is_empty() {
            return Ok(Vec::new());
        }
        let document_ids = document_ids.iter().map(|id| id.0).collect::<Vec<_>>();
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, document_id, chunk_index, content,
                   token_count, state, metadata, created_at, updated_at
            from document_chunks
            where tenant_id = $1 and document_id = any($2)
            order by document_id asc, chunk_index asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_ids)
        .bind(limit.clamp(1, 50_000) as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_document_chunk_row).collect()
    }

    pub async fn mark_indexed(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
        metadata_updates: &Value,
        updated_at: DateTime<Utc>,
    ) -> Result<Vec<DocumentChunk>> {
        let current_chunks = self.list_by_document(tenant_id, document_id).await?;
        let mut persisted = Vec::with_capacity(current_chunks.len());

        for chunk in current_chunks {
            let merged_metadata = merge_chunk_metadata(&chunk.metadata, metadata_updates)?;
            let row = sqlx::query(
                r#"
                update document_chunks
                set state = $4,
                    metadata = $5,
                    updated_at = $6
                where tenant_id = $1 and id = $2 and document_id = $3
                returning id, tenant_id, dataset_id, document_id, chunk_index, content, token_count, state, metadata, created_at, updated_at
                "#,
            )
            .bind(tenant_id.0)
            .bind(chunk.id.0)
            .bind(document_id.0)
            .bind(DocumentChunkState::Indexed.as_str())
            .bind(merged_metadata)
            .bind(updated_at)
            .fetch_one(&self.pool)
            .await?;

            persisted.push(map_document_chunk_row(&row)?);
        }

        Ok(persisted)
    }
}

#[derive(Clone)]
pub struct PgDocumentFactRepository {
    pool: PgPool,
}

impl PgDocumentFactRepository {
    pub async fn replace_document_facts(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
        facts: &[NewDocumentFact],
    ) -> Result<Vec<DocumentFact>> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            delete from document_facts
            where tenant_id = $1 and document_id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .execute(&mut *tx)
        .await?;

        let mut persisted = Vec::with_capacity(facts.len());
        for fact in facts {
            let fact_id = Uuid::new_v4();
            let row = sqlx::query(
                r#"
                insert into document_facts (
                    id,
                    tenant_id,
                    dataset_id,
                    document_id,
                    fact_type,
                    name,
                    normalized_name,
                    value_text,
                    value_number,
                    value_date,
                    attributes,
                    confidence,
                    source_kind,
                    source_locator,
                    source_chunk_id,
                    parse_version,
                    created_at
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                returning id, tenant_id, dataset_id, document_id, fact_type, name, normalized_name, value_text, value_number, value_date, attributes, confidence, source_kind, source_locator, source_chunk_id, parse_version, created_at
                "#,
            )
            .bind(fact_id)
            .bind(tenant_id.0)
            .bind(fact.dataset_id.0)
            .bind(fact.document_id.0)
            .bind(&fact.fact_type)
            .bind(&fact.name)
            .bind(&fact.normalized_name)
            .bind(&fact.value_text)
            .bind(fact.value_number)
            .bind(fact.value_date)
            .bind(&fact.attributes)
            .bind(fact.confidence)
            .bind(&fact.source_kind)
            .bind(&fact.source_locator)
            .bind(fact.source_chunk_id.map(|chunk_id| chunk_id.0))
            .bind(&fact.parse_version)
            .bind(fact.created_at)
            .fetch_one(&mut *tx)
            .await?;

            for source in &fact.sources {
                sqlx::query(
                    r#"
                    insert into document_fact_sources (
                        id,
                        fact_id,
                        tenant_id,
                        dataset_id,
                        document_id,
                        source_kind,
                        source_locator,
                        source_chunk_id,
                        attributes,
                        created_at
                    )
                    values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                    "#,
                )
                .bind(Uuid::new_v4())
                .bind(fact_id)
                .bind(tenant_id.0)
                .bind(fact.dataset_id.0)
                .bind(fact.document_id.0)
                .bind(&source.source_kind)
                .bind(&source.source_locator)
                .bind(source.source_chunk_id.map(|chunk_id| chunk_id.0))
                .bind(&source.attributes)
                .bind(source.created_at)
                .execute(&mut *tx)
                .await?;
            }

            persisted.push(map_document_fact_row(&row)?);
        }

        tx.commit().await?;
        Ok(persisted)
    }

    pub async fn list_document_facts_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        fact_type: &str,
        limit: i64,
    ) -> Result<Vec<DocumentFact>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, document_id, fact_type, name, normalized_name, value_text, value_number, value_date, attributes, confidence, source_kind, source_locator, source_chunk_id, parse_version, created_at
            from document_facts
            where tenant_id = $1
              and document_id in (
                select coalesce(canonical_document_id, id)
                from documents
                where tenant_id = $1 and dataset_id = $2
              )
              and fact_type = $3
            order by normalized_name asc, document_id asc, created_at asc
            limit $4
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(fact_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_document_fact_row).collect()
    }

    pub async fn aggregate_document_facts_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        fact_type: &str,
        limit: i64,
    ) -> Result<Vec<DocumentFactAggregate>> {
        let rows = sqlx::query(
            r#"
            select
                fact_type,
                normalized_name,
                min(name) as name,
                count(*)::bigint as fact_count,
                count(distinct document_id)::bigint as document_count,
                array_agg(distinct document_id) as source_document_ids,
                coalesce(
                    array_agg(distinct source_locator) filter (where source_locator is not null),
                    '{}'::text[]
                ) as source_locators
            from document_facts
            where tenant_id = $1
              and document_id in (
                select coalesce(canonical_document_id, id)
                from documents
                where tenant_id = $1 and dataset_id = $2
              )
              and fact_type = $3
            group by fact_type, normalized_name
            order by document_count desc, fact_count desc, normalized_name asc
            limit $4
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(fact_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| DocumentFactAggregate {
                fact_type: row.get("fact_type"),
                normalized_name: row.get("normalized_name"),
                name: row.get("name"),
                fact_count: row.get("fact_count"),
                document_count: row.get("document_count"),
                source_document_ids: row
                    .get::<Vec<Uuid>, _>("source_document_ids")
                    .into_iter()
                    .map(DocumentId)
                    .collect(),
                source_locators: row.get("source_locators"),
            })
            .collect())
    }

    pub async fn aggregate_document_facts_by_documents(
        &self,
        tenant_id: TenantId,
        document_ids: &[DocumentId],
        fact_type: &str,
        limit: i64,
    ) -> Result<Vec<DocumentFactAggregate>> {
        if document_ids.is_empty() {
            return Ok(Vec::new());
        }

        let raw_document_ids = document_ids
            .iter()
            .map(|document_id| document_id.0)
            .collect::<Vec<_>>();
        let rows = sqlx::query(
            r#"
            with effective_documents as (
                select distinct coalesce(canonical_document_id, id) as document_id
                from documents
                where tenant_id = $1 and id = any($2::uuid[])
            )
            select
                fact_type,
                normalized_name,
                min(name) as name,
                count(*)::bigint as fact_count,
                count(distinct document_id)::bigint as document_count,
                array_agg(distinct document_id) as source_document_ids,
                coalesce(
                    array_agg(distinct source_locator) filter (where source_locator is not null),
                    '{}'::text[]
                ) as source_locators
            from document_facts
            where tenant_id = $1
              and document_id in (select document_id from effective_documents)
              and fact_type = $3
            group by fact_type, normalized_name
            order by document_count desc, fact_count desc, normalized_name asc
            limit $4
            "#,
        )
        .bind(tenant_id.0)
        .bind(raw_document_ids)
        .bind(fact_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| DocumentFactAggregate {
                fact_type: row.get("fact_type"),
                normalized_name: row.get("normalized_name"),
                name: row.get("name"),
                fact_count: row.get("fact_count"),
                document_count: row.get("document_count"),
                source_document_ids: row
                    .get::<Vec<Uuid>, _>("source_document_ids")
                    .into_iter()
                    .map(DocumentId)
                    .collect(),
                source_locators: row.get("source_locators"),
            })
            .collect())
    }

    pub async fn list_by_document_ids_bounded(
        &self,
        tenant_id: TenantId,
        document_ids: &[DocumentId],
        limit: usize,
    ) -> Result<Vec<DocumentFact>> {
        if document_ids.is_empty() {
            return Ok(Vec::new());
        }
        let document_ids = document_ids.iter().map(|id| id.0).collect::<Vec<_>>();
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, document_id, fact_type, name,
                   normalized_name, value_text, value_number, value_date, attributes,
                   confidence, source_kind, source_locator, source_chunk_id,
                   parse_version, created_at
            from document_facts
            where tenant_id = $1 and document_id = any($2)
            order by document_id asc, fact_type asc, normalized_name asc, id asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_ids)
        .bind(limit.clamp(1, 50_000) as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_document_fact_row).collect()
    }
}

#[derive(Clone)]
pub struct PgDatasetFactSnapshotRepository {
    pool: PgPool,
}

impl PgDatasetFactSnapshotRepository {
    pub async fn upsert_dataset_fact_snapshot(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        snapshot_kind: &str,
        snapshot_key: &str,
        snapshot_manifest: &Value,
        source_fact_count: i64,
        source_document_count: i64,
        created_at: DateTime<Utc>,
    ) -> Result<DatasetFactSnapshot> {
        let row = sqlx::query(
            r#"
            insert into dataset_fact_snapshots (
                tenant_id,
                dataset_id,
                snapshot_kind,
                snapshot_key,
                snapshot_manifest,
                source_fact_count,
                source_document_count,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            on conflict (tenant_id, dataset_id, snapshot_kind, snapshot_key)
            do update set
                snapshot_manifest = excluded.snapshot_manifest,
                source_fact_count = excluded.source_fact_count,
                source_document_count = excluded.source_document_count,
                created_at = excluded.created_at
            returning tenant_id, dataset_id, snapshot_kind, snapshot_key, snapshot_manifest, source_fact_count, source_document_count, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(snapshot_kind)
        .bind(snapshot_key)
        .bind(snapshot_manifest)
        .bind(source_fact_count)
        .bind(source_document_count)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;

        map_dataset_fact_snapshot_row(&row)
    }

    pub async fn get_dataset_fact_snapshot(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        snapshot_kind: &str,
        snapshot_key: &str,
    ) -> Result<Option<DatasetFactSnapshot>> {
        let row = sqlx::query(
            r#"
            select tenant_id, dataset_id, snapshot_kind, snapshot_key, snapshot_manifest, source_fact_count, source_document_count, created_at
            from dataset_fact_snapshots
            where tenant_id = $1 and dataset_id = $2 and snapshot_kind = $3 and snapshot_key = $4
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(snapshot_kind)
        .bind(snapshot_key)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| map_dataset_fact_snapshot_row(&row))
            .transpose()
    }

    pub async fn load_latest_dataset_fact_snapshot(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        snapshot_kind: &str,
    ) -> Result<Option<DatasetFactSnapshot>> {
        let row = sqlx::query(
            r#"
            select tenant_id, dataset_id, snapshot_kind, snapshot_key, snapshot_manifest,
                   source_fact_count, source_document_count, created_at
            from dataset_fact_snapshots
            where tenant_id = $1 and dataset_id = $2 and snapshot_kind = $3
            order by created_at desc, snapshot_key desc
            limit 1
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(snapshot_kind)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| map_dataset_fact_snapshot_row(&row))
            .transpose()
    }
}

const DATASET_SEMANTIC_SNAPSHOT_COLUMNS: &str = r#"
    id, tenant_id, dataset_id, schema_version, generation_version,
    source_fingerprint, status, manifest, source_document_count,
    source_asset_count, source_record_count, node_count, edge_count,
    failure_code, generated_at, created_at, updated_at
"#;

const DATASET_SEMANTIC_BEGIN_BUILD_SQL: &str = r#"
    insert into dataset_semantic_snapshots (
        tenant_id, dataset_id, schema_version, generation_version,
        source_fingerprint, status, manifest, source_document_count,
        source_asset_count, source_record_count, created_at, updated_at
    )
    values ($1, $2, $3, $4, $5, 'building', $6, $7, $8, $9, $10, $10)
    on conflict (tenant_id, dataset_id, generation_version, source_fingerprint)
    do update set
        schema_version = excluded.schema_version,
        status = 'building',
        manifest = excluded.manifest,
        source_document_count = excluded.source_document_count,
        source_asset_count = excluded.source_asset_count,
        source_record_count = excluded.source_record_count,
        node_count = 0,
        edge_count = 0,
        failure_code = null,
        generated_at = null,
        updated_at = excluded.updated_at
    where dataset_semantic_snapshots.status in ('failed', 'superseded')
"#;

pub const DATASET_SEMANTIC_LATEST_READY_SQL: &str = r#"
    select id, tenant_id, dataset_id, schema_version, generation_version,
           source_fingerprint, status, manifest, source_document_count,
           source_asset_count, source_record_count, node_count, edge_count,
           failure_code, generated_at, created_at, updated_at
    from dataset_semantic_snapshots
    where tenant_id = $1 and dataset_id = $2 and status = 'ready'
    order by generated_at desc, created_at desc
    limit 1
"#;

pub const DATASET_SEMANTIC_READY_BY_ID_SQL: &str = r#"
    select id, tenant_id, dataset_id, schema_version, generation_version,
           source_fingerprint, status, manifest, source_document_count,
           source_asset_count, source_record_count, node_count, edge_count,
           failure_code, generated_at, created_at, updated_at
    from dataset_semantic_snapshots
    where tenant_id = $1 and dataset_id = $2 and id = $3 and status = 'ready'
    limit 1
"#;

pub const DATASET_SEMANTIC_LATEST_READY_BY_TENANT_SQL: &str = r#"
    select distinct on (dataset_id)
           id, tenant_id, dataset_id, schema_version, generation_version,
           source_fingerprint, status, manifest, source_document_count,
           source_asset_count, source_record_count, node_count, edge_count,
           failure_code, generated_at, created_at, updated_at
    from dataset_semantic_snapshots
    where tenant_id = $1 and status = 'ready'
    order by dataset_id asc, generated_at desc, created_at desc, id asc
    limit $2
"#;

const DATASET_SEMANTIC_MARK_READY_SQL: &str = r#"
    update dataset_semantic_snapshots
    set status = 'ready', manifest = $4, node_count = $5, edge_count = $6,
        failure_code = null, generated_at = $7, updated_at = $7
    where tenant_id = $1 and dataset_id = $2 and id = $3 and status = 'building'
    returning id, tenant_id, dataset_id, schema_version, generation_version,
              source_fingerprint, status, manifest, source_document_count,
              source_asset_count, source_record_count, node_count, edge_count,
              failure_code, generated_at, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_MARK_FAILED_SQL: &str = r#"
    update dataset_semantic_snapshots
    set status = 'failed', failure_code = $4, updated_at = $5
    where tenant_id = $1 and dataset_id = $2 and id = $3 and status = 'building'
    returning id, tenant_id, dataset_id, schema_version, generation_version,
              source_fingerprint, status, manifest, source_document_count,
              source_asset_count, source_record_count, node_count, edge_count,
              failure_code, generated_at, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_BEGIN_BUILD_SQL: &str = r#"
    insert into dataset_semantic_link_snapshots (
        tenant_id, left_dataset_id, right_dataset_id,
        left_snapshot_id, right_snapshot_id, schema_version,
        generation_version, source_fingerprint, status, manifest,
        created_at, updated_at
    )
    values ($1, $2, $3, $4, $5, $6, $7, $8, 'building', $9, $10, $10)
    on conflict (tenant_id, left_snapshot_id, right_snapshot_id, generation_version)
    do update set
        schema_version = excluded.schema_version,
        source_fingerprint = excluded.source_fingerprint,
        status = 'building',
        manifest = excluded.manifest,
        node_count = 0,
        edge_count = 0,
        failure_code = null,
        generated_at = null,
        updated_at = excluded.updated_at
    where dataset_semantic_link_snapshots.status in ('failed', 'superseded')
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, schema_version, generation_version,
              source_fingerprint, status, manifest, node_count, edge_count,
              failure_code, generated_at, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_LATEST_READY_SQL: &str = r#"
    select id, tenant_id, left_dataset_id, right_dataset_id,
           left_snapshot_id, right_snapshot_id, schema_version, generation_version,
           source_fingerprint, status, manifest, node_count, edge_count,
           failure_code, generated_at, created_at, updated_at
    from dataset_semantic_link_snapshots
    where tenant_id = $1
      and left_dataset_id = $2
      and right_dataset_id = $3
      and status = 'ready'
    order by generated_at desc, created_at desc, id asc
    limit 1
"#;

pub const DATASET_SEMANTIC_LINK_LATEST_ATTEMPT_SQL: &str = r#"
    select id, tenant_id, left_dataset_id, right_dataset_id,
           left_snapshot_id, right_snapshot_id, schema_version, generation_version,
           source_fingerprint, status, manifest, node_count, edge_count,
           failure_code, generated_at, created_at, updated_at
    from dataset_semantic_link_snapshots
    where tenant_id = $1
      and left_dataset_id = $2
      and right_dataset_id = $3
    order by updated_at desc, created_at desc, id asc
    limit 1
"#;

pub const DATASET_SEMANTIC_LINK_BY_INPUTS_SQL: &str = r#"
    select id, tenant_id, left_dataset_id, right_dataset_id,
           left_snapshot_id, right_snapshot_id, schema_version, generation_version,
           source_fingerprint, status, manifest, node_count, edge_count,
           failure_code, generated_at, created_at, updated_at
    from dataset_semantic_link_snapshots
    where tenant_id = $1
      and left_dataset_id = $2
      and right_dataset_id = $3
      and left_snapshot_id = $4
      and right_snapshot_id = $5
      and generation_version = $6
    limit 1
"#;

pub const DATASET_SEMANTIC_LINK_MARK_READY_SQL: &str = r#"
    update dataset_semantic_link_snapshots
    set status = 'ready', manifest = $5, node_count = $6, edge_count = $7,
        failure_code = null, generated_at = $8, updated_at = $8
    where tenant_id = $1
      and left_dataset_id = $2
      and right_dataset_id = $3
      and id = $4
      and status = 'building'
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, schema_version, generation_version,
              source_fingerprint, status, manifest, node_count, edge_count,
              failure_code, generated_at, created_at, updated_at
"#;

const DATASET_SEMANTIC_LINK_SUPERSEDE_PREVIOUS_SQL: &str = r#"
    update dataset_semantic_link_snapshots
    set status = 'superseded', updated_at = $5
    where tenant_id = $1
      and left_dataset_id = $2
      and right_dataset_id = $3
      and id <> $4
      and status = 'ready'
"#;

pub const DATASET_SEMANTIC_LINK_MARK_FAILED_SQL: &str = r#"
    update dataset_semantic_link_snapshots
    set status = 'failed', failure_code = $5, updated_at = $6
    where tenant_id = $1
      and left_dataset_id = $2
      and right_dataset_id = $3
      and id = $4
      and status = 'building'
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, schema_version, generation_version,
              source_fingerprint, status, manifest, node_count, edge_count,
              failure_code, generated_at, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_CREATE_OR_GET_RUN_SQL: &str = r#"
    insert into dataset_semantic_link_runs (
        tenant_id, left_dataset_id, right_dataset_id,
        left_snapshot_id, right_snapshot_id, generation_version,
        source_fingerprint, priority, max_attempts, available_at,
        created_at, updated_at
    )
    values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
    on conflict (tenant_id, left_snapshot_id, right_snapshot_id, generation_version)
    do update set
        priority = least(dataset_semantic_link_runs.priority, excluded.priority),
        max_attempts = greatest(dataset_semantic_link_runs.max_attempts, excluded.max_attempts),
        available_at = case
            when dataset_semantic_link_runs.status in ('pending', 'retry_wait')
                then least(dataset_semantic_link_runs.available_at, excluded.available_at)
            else dataset_semantic_link_runs.available_at
        end,
        updated_at = excluded.updated_at
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, generation_version,
              source_fingerprint, status, priority, attempt_count, max_attempts,
              available_at, claimed_at, finished_at, failure_code, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_SQL: &str = r#"
    with next_run as (
        select id
        from dataset_semantic_link_runs
        where tenant_id = $1
          and status in ('pending', 'retry_wait')
          and available_at <= $2
          and attempt_count < max_attempts
        order by priority asc, available_at asc, created_at asc, id asc
        for update skip locked
        limit 1
    )
    update dataset_semantic_link_runs
    set status = 'running',
        attempt_count = attempt_count + 1,
        claimed_at = $2,
        finished_at = null,
        failure_code = null,
        updated_at = $2
    where tenant_id = $1 and id in (select id from next_run)
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, generation_version,
              source_fingerprint, status, priority, attempt_count, max_attempts,
              available_at, claimed_at, finished_at, failure_code, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_FOR_DATASETS_SQL: &str = r#"
    with next_run as (
        select id
        from dataset_semantic_link_runs
        where tenant_id = $1
          and status in ('pending', 'retry_wait')
          and available_at <= $2
          and attempt_count < max_attempts
          and left_dataset_id = any($3::uuid[])
          and right_dataset_id = any($3::uuid[])
        order by priority asc, available_at asc, created_at asc, id asc
        for update skip locked
        limit 1
    )
    update dataset_semantic_link_runs
    set status = 'running',
        attempt_count = attempt_count + 1,
        claimed_at = $2,
        finished_at = null,
        failure_code = null,
        updated_at = $2
    where tenant_id = $1 and id in (select id from next_run)
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, generation_version,
              source_fingerprint, status, priority, attempt_count, max_attempts,
              available_at, claimed_at, finished_at, failure_code, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_DEFER_RUN_SQL: &str = r#"
    update dataset_semantic_link_runs
    set status = 'retry_wait',
        attempt_count = greatest(attempt_count - 1, 0),
        available_at = $3,
        claimed_at = null,
        finished_at = null,
        failure_code = null,
        updated_at = $4
    where tenant_id = $1 and id = $2 and status = 'running'
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, generation_version,
              source_fingerprint, status, priority, attempt_count, max_attempts,
              available_at, claimed_at, finished_at, failure_code, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_RECOVER_STALE_RUNS_SQL: &str = r#"
    update dataset_semantic_link_runs
    set status = case
            when attempt_count >= max_attempts then 'dead_letter'
            else 'retry_wait'
        end,
        available_at = $3,
        claimed_at = null,
        finished_at = case
            when attempt_count >= max_attempts then $3
            else null
        end,
        failure_code = 'worker_lease_expired',
        updated_at = $3
    where tenant_id = $1
      and status = 'running'
      and claimed_at <= $2
      and left_dataset_id = any($4::uuid[])
      and right_dataset_id = any($4::uuid[])
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, generation_version,
              source_fingerprint, status, priority, attempt_count, max_attempts,
              available_at, claimed_at, finished_at, failure_code, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_FAIL_BUILD_FOR_RUN_SQL: &str = r#"
    update dataset_semantic_link_snapshots
    set status = 'failed', failure_code = $6, updated_at = $7
    where tenant_id = $1
      and left_dataset_id = $2
      and right_dataset_id = $3
      and left_snapshot_id = $4
      and right_snapshot_id = $5
      and status = 'building'
"#;

pub const DATASET_SEMANTIC_LINK_MARK_PAIR_READY_STALE_SQL: &str = r#"
    update dataset_semantic_link_snapshots
    set manifest = jsonb_set(manifest, '{stale}', 'true'::jsonb, true),
        updated_at = $4
    where tenant_id = $1
      and left_dataset_id = $2
      and right_dataset_id = $3
      and status = 'ready'
"#;

pub const DATASET_SEMANTIC_LINK_MARK_DATASET_READY_STALE_SQL: &str = r#"
    update dataset_semantic_link_snapshots
    set manifest = jsonb_set(manifest, '{stale}', 'true'::jsonb, true),
        updated_at = $3
    where tenant_id = $1
      and status = 'ready'
      and (left_dataset_id = $2 or right_dataset_id = $2)
"#;

pub const DATASET_SEMANTIC_LINK_REVIVE_DEAD_LETTER_RUN_SQL: &str = r#"
    update dataset_semantic_link_runs
    set status = 'pending', attempt_count = 0, available_at = $3,
        claimed_at = null, finished_at = null, failure_code = null, updated_at = $3
    where tenant_id = $1 and id = $2 and status = 'dead_letter'
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, generation_version,
              source_fingerprint, status, priority, attempt_count, max_attempts,
              available_at, claimed_at, finished_at, failure_code, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_RETRY_OR_DEAD_LETTER_SQL: &str = r#"
    update dataset_semantic_link_runs
    set status = case
            when attempt_count >= max_attempts then 'dead_letter'
            else 'retry_wait'
        end,
        available_at = $4,
        claimed_at = null,
        finished_at = case
            when attempt_count >= max_attempts then $5
            else null
        end,
        failure_code = $3,
        updated_at = $5
    where tenant_id = $1 and id = $2 and status = 'running'
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, generation_version,
              source_fingerprint, status, priority, attempt_count, max_attempts,
              available_at, claimed_at, finished_at, failure_code, created_at, updated_at
"#;

pub const DATASET_SEMANTIC_LINK_MARK_RUN_SUCCEEDED_SQL: &str = r#"
    update dataset_semantic_link_runs
    set status = 'succeeded', finished_at = $3, failure_code = null, updated_at = $3
    where tenant_id = $1 and id = $2 and status = 'running'
    returning id, tenant_id, left_dataset_id, right_dataset_id,
              left_snapshot_id, right_snapshot_id, generation_version,
              source_fingerprint, status, priority, attempt_count, max_attempts,
              available_at, claimed_at, finished_at, failure_code, created_at, updated_at
"#;

#[derive(Clone)]
pub struct PgDatasetSemanticSnapshotRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgDatasetSemanticLinkRepository {
    pool: PgPool,
}

pub fn canonical_dataset_semantic_link_pair(
    left_dataset_id: DatasetId,
    right_dataset_id: DatasetId,
    left_snapshot_id: Uuid,
    right_snapshot_id: Uuid,
) -> Result<(DatasetId, DatasetId, Uuid, Uuid)> {
    if left_dataset_id == right_dataset_id {
        return Err(anyhow!("dataset semantic link endpoints must be distinct"));
    }
    if left_snapshot_id == right_snapshot_id {
        return Err(anyhow!("dataset semantic link snapshots must be distinct"));
    }
    if left_dataset_id < right_dataset_id {
        Ok((
            left_dataset_id,
            right_dataset_id,
            left_snapshot_id,
            right_snapshot_id,
        ))
    } else {
        Ok((
            right_dataset_id,
            left_dataset_id,
            right_snapshot_id,
            left_snapshot_id,
        ))
    }
}

fn canonical_dataset_semantic_link_dataset_pair(
    left_dataset_id: DatasetId,
    right_dataset_id: DatasetId,
) -> Result<(DatasetId, DatasetId)> {
    if left_dataset_id == right_dataset_id {
        return Err(anyhow!("dataset semantic link endpoints must be distinct"));
    }
    Ok(if left_dataset_id < right_dataset_id {
        (left_dataset_id, right_dataset_id)
    } else {
        (right_dataset_id, left_dataset_id)
    })
}

fn dataset_semantic_link_run_status_requires_failed_build(status: &str) -> bool {
    status == "dead_letter"
}

fn validate_dataset_semantic_link_manifest(manifest: &Value) -> Result<()> {
    if !manifest.is_object() {
        return Err(anyhow!(
            "dataset semantic link manifest must be a JSON object"
        ));
    }
    Ok(())
}

fn validate_dataset_semantic_link_ready_payload(
    manifest: &Value,
    node_count: i32,
    edge_count: i32,
) -> Result<()> {
    validate_dataset_semantic_link_manifest(manifest)?;
    if node_count < 0 || edge_count < 0 {
        return Err(anyhow!(
            "dataset semantic link node and edge counts must be non-negative"
        ));
    }
    Ok(())
}

impl PgDatasetSemanticSnapshotRepository {
    pub async fn try_begin_build(
        &self,
        tenant_id: TenantId,
        snapshot: &NewDatasetSemanticSnapshot,
        now: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticSnapshot>> {
        let row = sqlx::query(
            r#"
            insert into dataset_semantic_snapshots (
                tenant_id, dataset_id, schema_version, generation_version,
                source_fingerprint, status, manifest, source_document_count,
                source_asset_count, source_record_count, created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, 'building', $6, $7, $8, $9, $10, $10)
            on conflict (tenant_id, dataset_id, generation_version, source_fingerprint)
            do update set status = 'building', manifest = excluded.manifest,
                source_document_count = excluded.source_document_count,
                source_asset_count = excluded.source_asset_count,
                source_record_count = excluded.source_record_count,
                node_count = 0, edge_count = 0, failure_code = null,
                generated_at = null, updated_at = excluded.updated_at
            where dataset_semantic_snapshots.status in ('failed', 'superseded')
            returning id, tenant_id, dataset_id, schema_version, generation_version,
                      source_fingerprint, status, manifest, source_document_count,
                      source_asset_count, source_record_count, node_count, edge_count,
                      failure_code, generated_at, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(snapshot.dataset_id.0)
        .bind(&snapshot.schema_version)
        .bind(&snapshot.generation_version)
        .bind(&snapshot.source_fingerprint)
        .bind(&snapshot.manifest)
        .bind(snapshot.source_document_count)
        .bind(snapshot.source_asset_count)
        .bind(snapshot.source_record_count)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| map_dataset_semantic_snapshot_row(&row))
            .transpose()
    }

    pub async fn begin_build(
        &self,
        tenant_id: TenantId,
        snapshot: NewDatasetSemanticSnapshot,
        now: DateTime<Utc>,
    ) -> Result<DatasetSemanticSnapshot> {
        sqlx::query(DATASET_SEMANTIC_BEGIN_BUILD_SQL)
            .bind(tenant_id.0)
            .bind(snapshot.dataset_id.0)
            .bind(&snapshot.schema_version)
            .bind(&snapshot.generation_version)
            .bind(&snapshot.source_fingerprint)
            .bind(&snapshot.manifest)
            .bind(snapshot.source_document_count)
            .bind(snapshot.source_asset_count)
            .bind(snapshot.source_record_count)
            .bind(now)
            .execute(&self.pool)
            .await?;

        let sql = format!(
            "select {DATASET_SEMANTIC_SNAPSHOT_COLUMNS} from dataset_semantic_snapshots \
             where tenant_id = $1 and dataset_id = $2 and generation_version = $3 \
             and source_fingerprint = $4"
        );
        let row = sqlx::query(AssertSqlSafe(sql))
            .bind(tenant_id.0)
            .bind(snapshot.dataset_id.0)
            .bind(snapshot.generation_version)
            .bind(snapshot.source_fingerprint)
            .fetch_one(&self.pool)
            .await?;
        map_dataset_semantic_snapshot_row(&row)
    }

    pub async fn mark_ready(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        snapshot_id: Uuid,
        manifest: &Value,
        node_count: i32,
        edge_count: i32,
        generated_at: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticSnapshot>> {
        let row = sqlx::query(DATASET_SEMANTIC_MARK_READY_SQL)
            .bind(tenant_id.0)
            .bind(dataset_id.0)
            .bind(snapshot_id)
            .bind(manifest)
            .bind(node_count)
            .bind(edge_count)
            .bind(generated_at)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_snapshot_row(&row))
            .transpose()
    }

    pub async fn mark_failed(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        snapshot_id: Uuid,
        failure_code: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticSnapshot>> {
        let row = sqlx::query(DATASET_SEMANTIC_MARK_FAILED_SQL)
            .bind(tenant_id.0)
            .bind(dataset_id.0)
            .bind(snapshot_id)
            .bind(failure_code)
            .bind(now)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_snapshot_row(&row))
            .transpose()
    }

    pub async fn load_latest_ready(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Option<DatasetSemanticSnapshot>> {
        let row = sqlx::query(DATASET_SEMANTIC_LATEST_READY_SQL)
            .bind(tenant_id.0)
            .bind(dataset_id.0)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_snapshot_row(&row))
            .transpose()
    }

    pub async fn load_ready_by_id(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        snapshot_id: Uuid,
    ) -> Result<Option<DatasetSemanticSnapshot>> {
        let row = sqlx::query(DATASET_SEMANTIC_READY_BY_ID_SQL)
            .bind(tenant_id.0)
            .bind(dataset_id.0)
            .bind(snapshot_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_snapshot_row(&row))
            .transpose()
    }

    pub async fn list_latest_ready_by_tenant(
        &self,
        tenant_id: TenantId,
        limit: usize,
    ) -> Result<Vec<DatasetSemanticSnapshot>> {
        let rows = sqlx::query(DATASET_SEMANTIC_LATEST_READY_BY_TENANT_SQL)
            .bind(tenant_id.0)
            .bind(limit.clamp(1, 10_000) as i64)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(map_dataset_semantic_snapshot_row).collect()
    }

    pub async fn load_latest_attempt(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Option<DatasetSemanticSnapshot>> {
        let sql = format!(
            "select {DATASET_SEMANTIC_SNAPSHOT_COLUMNS} from dataset_semantic_snapshots \
             where tenant_id = $1 and dataset_id = $2 \
             order by updated_at desc, created_at desc limit 1"
        );
        let row = sqlx::query(AssertSqlSafe(sql))
            .bind(tenant_id.0)
            .bind(dataset_id.0)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_snapshot_row(&row))
            .transpose()
    }
}

impl PgDatasetSemanticLinkRepository {
    pub async fn try_begin_build(
        &self,
        tenant_id: TenantId,
        snapshot: &NewDatasetSemanticLinkSnapshot,
        now: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkSnapshot>> {
        validate_dataset_semantic_link_manifest(&snapshot.manifest)?;
        let (left_dataset_id, right_dataset_id, left_snapshot_id, right_snapshot_id) =
            canonical_dataset_semantic_link_pair(
                snapshot.left_dataset_id,
                snapshot.right_dataset_id,
                snapshot.left_snapshot_id,
                snapshot.right_snapshot_id,
            )?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_BEGIN_BUILD_SQL)
            .bind(tenant_id.0)
            .bind(left_dataset_id.0)
            .bind(right_dataset_id.0)
            .bind(left_snapshot_id)
            .bind(right_snapshot_id)
            .bind(snapshot.schema_version.trim())
            .bind(snapshot.generation_version.trim())
            .bind(snapshot.source_fingerprint.trim())
            .bind(&snapshot.manifest)
            .bind(now)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_snapshot_row(&row))
            .transpose()
    }

    pub async fn load_latest_ready(
        &self,
        tenant_id: TenantId,
        left_dataset_id: DatasetId,
        right_dataset_id: DatasetId,
    ) -> Result<Option<DatasetSemanticLinkSnapshot>> {
        let (left_dataset_id, right_dataset_id) =
            canonical_dataset_semantic_link_dataset_pair(left_dataset_id, right_dataset_id)?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_LATEST_READY_SQL)
            .bind(tenant_id.0)
            .bind(left_dataset_id.0)
            .bind(right_dataset_id.0)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_snapshot_row(&row))
            .transpose()
    }

    pub async fn load_latest_attempt(
        &self,
        tenant_id: TenantId,
        left_dataset_id: DatasetId,
        right_dataset_id: DatasetId,
    ) -> Result<Option<DatasetSemanticLinkSnapshot>> {
        let (left_dataset_id, right_dataset_id) =
            canonical_dataset_semantic_link_dataset_pair(left_dataset_id, right_dataset_id)?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_LATEST_ATTEMPT_SQL)
            .bind(tenant_id.0)
            .bind(left_dataset_id.0)
            .bind(right_dataset_id.0)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_snapshot_row(&row))
            .transpose()
    }

    pub async fn load_by_inputs(
        &self,
        tenant_id: TenantId,
        left_dataset_id: DatasetId,
        right_dataset_id: DatasetId,
        left_snapshot_id: Uuid,
        right_snapshot_id: Uuid,
        generation_version: &str,
    ) -> Result<Option<DatasetSemanticLinkSnapshot>> {
        let (left_dataset_id, right_dataset_id, left_snapshot_id, right_snapshot_id) =
            canonical_dataset_semantic_link_pair(
                left_dataset_id,
                right_dataset_id,
                left_snapshot_id,
                right_snapshot_id,
            )?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_BY_INPUTS_SQL)
            .bind(tenant_id.0)
            .bind(left_dataset_id.0)
            .bind(right_dataset_id.0)
            .bind(left_snapshot_id)
            .bind(right_snapshot_id)
            .bind(generation_version.trim())
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_snapshot_row(&row))
            .transpose()
    }

    pub async fn mark_ready(
        &self,
        tenant_id: TenantId,
        left_dataset_id: DatasetId,
        right_dataset_id: DatasetId,
        link_snapshot_id: Uuid,
        manifest: &Value,
        node_count: i32,
        edge_count: i32,
        generated_at: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkSnapshot>> {
        validate_dataset_semantic_link_ready_payload(manifest, node_count, edge_count)?;
        let (left_dataset_id, right_dataset_id) =
            canonical_dataset_semantic_link_dataset_pair(left_dataset_id, right_dataset_id)?;
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_MARK_READY_SQL)
            .bind(tenant_id.0)
            .bind(left_dataset_id.0)
            .bind(right_dataset_id.0)
            .bind(link_snapshot_id)
            .bind(manifest)
            .bind(node_count)
            .bind(edge_count)
            .bind(generated_at)
            .fetch_optional(&mut *transaction)
            .await?;
        if row.is_some() {
            sqlx::query(DATASET_SEMANTIC_LINK_SUPERSEDE_PREVIOUS_SQL)
                .bind(tenant_id.0)
                .bind(left_dataset_id.0)
                .bind(right_dataset_id.0)
                .bind(link_snapshot_id)
                .bind(generated_at)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        row.map(|row| map_dataset_semantic_link_snapshot_row(&row))
            .transpose()
    }

    pub async fn mark_failed(
        &self,
        tenant_id: TenantId,
        left_dataset_id: DatasetId,
        right_dataset_id: DatasetId,
        link_snapshot_id: Uuid,
        failure_code: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkSnapshot>> {
        let (left_dataset_id, right_dataset_id) =
            canonical_dataset_semantic_link_dataset_pair(left_dataset_id, right_dataset_id)?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_MARK_FAILED_SQL)
            .bind(tenant_id.0)
            .bind(left_dataset_id.0)
            .bind(right_dataset_id.0)
            .bind(link_snapshot_id)
            .bind(failure_code.trim())
            .bind(now)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_snapshot_row(&row))
            .transpose()
    }

    pub async fn create_or_get_run(
        &self,
        tenant_id: TenantId,
        run: &NewDatasetSemanticLinkRun,
        now: DateTime<Utc>,
    ) -> Result<DatasetSemanticLinkRun> {
        if run.max_attempts <= 0 {
            return Err(anyhow!(
                "dataset semantic link run max_attempts must be positive"
            ));
        }
        let (left_dataset_id, right_dataset_id, left_snapshot_id, right_snapshot_id) =
            canonical_dataset_semantic_link_pair(
                run.left_dataset_id,
                run.right_dataset_id,
                run.left_snapshot_id,
                run.right_snapshot_id,
            )?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_CREATE_OR_GET_RUN_SQL)
            .bind(tenant_id.0)
            .bind(left_dataset_id.0)
            .bind(right_dataset_id.0)
            .bind(left_snapshot_id)
            .bind(right_snapshot_id)
            .bind(run.generation_version.trim())
            .bind(run.source_fingerprint.trim())
            .bind(run.priority)
            .bind(run.max_attempts)
            .bind(run.available_at)
            .bind(now)
            .fetch_one(&self.pool)
            .await?;
        map_dataset_semantic_link_run_row(&row)
    }

    pub async fn claim_ready_run(
        &self,
        tenant_id: TenantId,
        claimed_at: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkRun>> {
        let row = sqlx::query(DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_SQL)
            .bind(tenant_id.0)
            .bind(claimed_at)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_run_row(&row))
            .transpose()
    }

    pub async fn claim_ready_run_for_datasets(
        &self,
        tenant_id: TenantId,
        claimed_at: DateTime<Utc>,
        allowed_dataset_ids: &[DatasetId],
    ) -> Result<Option<DatasetSemanticLinkRun>> {
        if allowed_dataset_ids.is_empty() {
            return Ok(None);
        }
        let allowed_dataset_ids = allowed_dataset_ids
            .iter()
            .map(|dataset_id| dataset_id.0)
            .collect::<Vec<_>>();
        let row = sqlx::query(DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_FOR_DATASETS_SQL)
            .bind(tenant_id.0)
            .bind(claimed_at)
            .bind(allowed_dataset_ids)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_run_row(&row))
            .transpose()
    }

    pub async fn defer_run_without_attempt(
        &self,
        tenant_id: TenantId,
        run_id: Uuid,
        available_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkRun>> {
        let row = sqlx::query(DATASET_SEMANTIC_LINK_DEFER_RUN_SQL)
            .bind(tenant_id.0)
            .bind(run_id)
            .bind(available_at)
            .bind(now)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_run_row(&row))
            .transpose()
    }

    pub async fn recover_stale_runs(
        &self,
        tenant_id: TenantId,
        claimed_before: DateTime<Utc>,
        now: DateTime<Utc>,
        allowed_dataset_ids: &[DatasetId],
    ) -> Result<Vec<DatasetSemanticLinkRun>> {
        if allowed_dataset_ids.is_empty() {
            return Ok(Vec::new());
        }
        let allowed_dataset_ids = allowed_dataset_ids
            .iter()
            .map(|dataset_id| dataset_id.0)
            .collect::<Vec<_>>();
        let mut transaction = self.pool.begin().await?;
        let rows = sqlx::query(DATASET_SEMANTIC_LINK_RECOVER_STALE_RUNS_SQL)
            .bind(tenant_id.0)
            .bind(claimed_before)
            .bind(now)
            .bind(allowed_dataset_ids)
            .fetch_all(&mut *transaction)
            .await?;
        let runs = rows
            .iter()
            .map(map_dataset_semantic_link_run_row)
            .collect::<Result<Vec<_>>>()?;
        for run in &runs {
            let failed_build_count = sqlx::query(DATASET_SEMANTIC_LINK_FAIL_BUILD_FOR_RUN_SQL)
                .bind(tenant_id.0)
                .bind(run.left_dataset_id.0)
                .bind(run.right_dataset_id.0)
                .bind(run.left_snapshot_id)
                .bind(run.right_snapshot_id)
                .bind("worker_lease_expired")
                .bind(now)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            if failed_build_count > 0 {
                sqlx::query(DATASET_SEMANTIC_LINK_MARK_PAIR_READY_STALE_SQL)
                    .bind(tenant_id.0)
                    .bind(run.left_dataset_id.0)
                    .bind(run.right_dataset_id.0)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await?;
            }
        }
        transaction.commit().await?;
        Ok(runs)
    }

    pub async fn mark_ready_links_stale_for_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        Ok(
            sqlx::query(DATASET_SEMANTIC_LINK_MARK_DATASET_READY_STALE_SQL)
                .bind(tenant_id.0)
                .bind(dataset_id.0)
                .bind(now)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    pub async fn revive_dead_letter_run(
        &self,
        tenant_id: TenantId,
        run_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkRun>> {
        let row = sqlx::query(DATASET_SEMANTIC_LINK_REVIVE_DEAD_LETTER_RUN_SQL)
            .bind(tenant_id.0)
            .bind(run_id)
            .bind(now)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_run_row(&row))
            .transpose()
    }

    pub async fn retry_or_dead_letter(
        &self,
        tenant_id: TenantId,
        run: &DatasetSemanticLinkRun,
        failure_code: &str,
        available_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkRun>> {
        if run.tenant_id != tenant_id {
            return Err(anyhow!("dataset semantic link run tenant mismatch"));
        }
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_RETRY_OR_DEAD_LETTER_SQL)
            .bind(tenant_id.0)
            .bind(run.id)
            .bind(failure_code.trim())
            .bind(available_at)
            .bind(now)
            .fetch_optional(&mut *transaction)
            .await?;
        let transitioned = row
            .map(|row| map_dataset_semantic_link_run_row(&row))
            .transpose()?;
        if transitioned.as_ref().is_some_and(|transitioned| {
            dataset_semantic_link_run_status_requires_failed_build(&transitioned.status)
        }) {
            sqlx::query(DATASET_SEMANTIC_LINK_FAIL_BUILD_FOR_RUN_SQL)
                .bind(tenant_id.0)
                .bind(run.left_dataset_id.0)
                .bind(run.right_dataset_id.0)
                .bind(run.left_snapshot_id)
                .bind(run.right_snapshot_id)
                .bind(failure_code.trim())
                .bind(now)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(transitioned)
    }

    pub async fn mark_obsolete_run_succeeded(
        &self,
        tenant_id: TenantId,
        run: &DatasetSemanticLinkRun,
        failure_code: &str,
        finished_at: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkRun>> {
        if run.tenant_id != tenant_id {
            return Err(anyhow!("dataset semantic link run tenant mismatch"));
        }
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(DATASET_SEMANTIC_LINK_MARK_RUN_SUCCEEDED_SQL)
            .bind(tenant_id.0)
            .bind(run.id)
            .bind(finished_at)
            .fetch_optional(&mut *transaction)
            .await?;
        let transitioned = row
            .map(|row| map_dataset_semantic_link_run_row(&row))
            .transpose()?;
        if transitioned.is_some() {
            sqlx::query(DATASET_SEMANTIC_LINK_FAIL_BUILD_FOR_RUN_SQL)
                .bind(tenant_id.0)
                .bind(run.left_dataset_id.0)
                .bind(run.right_dataset_id.0)
                .bind(run.left_snapshot_id)
                .bind(run.right_snapshot_id)
                .bind(failure_code.trim())
                .bind(finished_at)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(transitioned)
    }

    pub async fn mark_run_succeeded(
        &self,
        tenant_id: TenantId,
        run_id: Uuid,
        finished_at: DateTime<Utc>,
    ) -> Result<Option<DatasetSemanticLinkRun>> {
        let row = sqlx::query(DATASET_SEMANTIC_LINK_MARK_RUN_SUCCEEDED_SQL)
            .bind(tenant_id.0)
            .bind(run_id)
            .bind(finished_at)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_dataset_semantic_link_run_row(&row))
            .transpose()
    }
}

const SEMANTIC_DICTIONARY_UPSERT_SQL: &str = r#"
    insert into semantic_dictionary_entries (
        tenant_id, source_kind, source_system_key, source_object_key,
        raw_field_key, display_name, description, semantic_role, value_type,
        status, confidence, created_by_user_id, created_at, updated_at
    )
    values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $13)
    on conflict (tenant_id, source_kind, source_system_key, source_object_key, raw_field_key)
    do update set
        display_name = excluded.display_name,
        description = excluded.description,
        semantic_role = excluded.semantic_role,
        value_type = excluded.value_type,
        status = excluded.status,
        confidence = excluded.confidence,
        created_by_user_id = coalesce(excluded.created_by_user_id, semantic_dictionary_entries.created_by_user_id),
        updated_at = excluded.updated_at
    returning id, tenant_id, source_kind, source_system_key, source_object_key,
              raw_field_key, display_name, description, semantic_role, value_type,
              status, confidence, created_by_user_id, created_at, updated_at
"#;

pub const SEMANTIC_DICTIONARY_RESOLVE_SQL: &str = r#"
    select id, tenant_id, source_kind, source_system_key, source_object_key,
           raw_field_key, display_name, description, semantic_role, value_type,
           status, confidence, created_by_user_id, created_at, updated_at
    from semantic_dictionary_entries
    where tenant_id = $1
      and source_kind = $2
      and raw_field_key = $5
      and source_system_key in ($3, '*')
      and source_object_key in ($4, '*')
      and status in ('confirmed', 'suggested')
    order by
      case status when 'confirmed' then 0 else 1 end,
      case when source_system_key = $3 then 0 else 1 end,
      case when source_object_key = $4 then 0 else 1 end,
      confidence desc,
      updated_at desc
    limit 1
"#;

#[derive(Clone)]
pub struct PgSemanticDictionaryRepository {
    pool: PgPool,
}

impl PgSemanticDictionaryRepository {
    pub async fn list_for_resolution(
        &self,
        tenant_id: TenantId,
        limit: usize,
    ) -> Result<Vec<SemanticDictionaryEntry>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, source_kind, source_system_key, source_object_key,
                   raw_field_key, display_name, description, semantic_role, value_type,
                   status, confidence, created_by_user_id, created_at, updated_at
            from semantic_dictionary_entries
            where tenant_id = $1 and status in ('confirmed', 'suggested')
            order by case status when 'confirmed' then 0 else 1 end,
                     updated_at desc, id asc
            limit $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(limit.clamp(1, 10_000) as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_semantic_dictionary_entry_row).collect()
    }

    pub async fn upsert(
        &self,
        tenant_id: TenantId,
        mut entry: NewSemanticDictionaryEntry,
        now: DateTime<Utc>,
    ) -> Result<SemanticDictionaryEntry> {
        entry.source_system_key = normalized_dictionary_scope_key(&entry.source_system_key);
        entry.source_object_key = normalized_dictionary_scope_key(&entry.source_object_key);
        let row = sqlx::query(SEMANTIC_DICTIONARY_UPSERT_SQL)
            .bind(tenant_id.0)
            .bind(entry.source_kind.trim())
            .bind(entry.source_system_key)
            .bind(entry.source_object_key)
            .bind(entry.raw_field_key.trim())
            .bind(entry.display_name.trim())
            .bind(entry.description)
            .bind(entry.semantic_role.trim())
            .bind(entry.value_type.trim())
            .bind(entry.status.trim())
            .bind(entry.confidence.clamp(0.0, 1.0))
            .bind(entry.created_by_user_id.map(|id| id.0))
            .bind(now)
            .fetch_one(&self.pool)
            .await?;
        map_semantic_dictionary_entry_row(&row)
    }

    pub async fn resolve(
        &self,
        tenant_id: TenantId,
        source_kind: &str,
        source_system_key: &str,
        source_object_key: &str,
        raw_field_key: &str,
    ) -> Result<Option<SemanticDictionaryEntry>> {
        let row = sqlx::query(SEMANTIC_DICTIONARY_RESOLVE_SQL)
            .bind(tenant_id.0)
            .bind(source_kind.trim())
            .bind(normalized_dictionary_scope_key(source_system_key))
            .bind(normalized_dictionary_scope_key(source_object_key))
            .bind(raw_field_key.trim())
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| map_semantic_dictionary_entry_row(&row))
            .transpose()
    }
}

fn normalized_dictionary_scope_key(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "*".to_string()
    } else {
        value.to_string()
    }
}

#[derive(Clone)]
pub struct PgReportPlanRepository {
    pool: PgPool,
}

impl PgReportPlanRepository {
    pub async fn create(&self, tenant_id: TenantId, new_plan: NewReportPlan) -> Result<ReportPlan> {
        let row = sqlx::query(
            r#"
            insert into report_plans (tenant_id, dataset_id, owner_user_id, title, objective, status, theme_key, ast)
            values ($1, $2, $3, $4, $5, $6, $7, '{}'::jsonb)
            returning id, tenant_id, dataset_id, owner_user_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_plan.dataset_id.0)
        .bind(new_plan.owner_user_id.map(|id| id.0))
        .bind(new_plan.title)
        .bind(new_plan.objective)
        .bind(ReportPlanStatus::Draft.as_str())
        .bind(new_plan.theme_key)
        .fetch_one(&self.pool)
        .await?;

        map_report_plan_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        plan_id: ReportPlanId,
    ) -> Result<Option<ReportPlan>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, owner_user_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
            from report_plans
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_report_plan_row).transpose()
    }

    pub async fn list_by_tenant(&self, tenant_id: TenantId) -> Result<Vec<ReportPlan>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, owner_user_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
            from report_plans
            where tenant_id = $1
            order by created_at desc, title asc
            "#,
        )
        .bind(tenant_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_report_plan_row).collect()
    }

    pub async fn mark_planned(
        &self,
        tenant_id: TenantId,
        plan_id: ReportPlanId,
        current_ast_version_id: ReportPlanAstVersionId,
        ast: &Value,
        updated_at: DateTime<Utc>,
    ) -> Result<ReportPlan> {
        let row = sqlx::query(
            r#"
            update report_plans
            set status = $3,
                current_ast_version_id = $4,
                ast = $5,
                updated_at = $6
            where tenant_id = $1 and id = $2
            returning id, tenant_id, dataset_id, owner_user_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .bind(ReportPlanStatus::Planned.as_str())
        .bind(current_ast_version_id.0)
        .bind(ast)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_report_plan_row(&row)
    }

    pub async fn mark_published(
        &self,
        tenant_id: TenantId,
        plan_id: ReportPlanId,
        updated_at: DateTime<Utc>,
    ) -> Result<ReportPlan> {
        let row = sqlx::query(
            r#"
            update report_plans
            set status = $3,
                updated_at = $4
            where tenant_id = $1 and id = $2
            returning id, tenant_id, dataset_id, owner_user_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .bind(ReportPlanStatus::Published.as_str())
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_report_plan_row(&row)
    }
}

#[derive(Clone)]
pub struct PgReportPlanAstVersionRepository {
    pool: PgPool,
}

impl PgReportPlanAstVersionRepository {
    pub async fn create_next_version(
        &self,
        tenant_id: TenantId,
        plan_id: ReportPlanId,
        new_version: &NewReportPlanAstVersion,
    ) -> Result<ReportPlanAstVersion> {
        let mut tx = self.pool.begin().await?;
        let plan_exists = sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
                select 1
                from report_plans
                where tenant_id = $1 and id = $2
            )
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .fetch_one(&mut *tx)
        .await?;
        if !plan_exists {
            return Err(anyhow!(
                "report plan {} was not found for tenant {}",
                plan_id,
                tenant_id
            ));
        }

        let next_version_no = sqlx::query_scalar::<_, i32>(
            r#"
            select coalesce(max(v.version_no), 0) + 1
            from report_plan_ast_versions v
            join report_plans p on p.id = v.plan_id
            where p.tenant_id = $1 and v.plan_id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .fetch_one(&mut *tx)
        .await?;

        let version = ReportPlanAstVersion {
            id: ReportPlanAstVersionId::new(),
            plan_id,
            version_no: next_version_no,
            ast: new_version.ast.clone(),
            created_at: new_version.created_at,
        };

        sqlx::query(
            r#"
            insert into report_plan_ast_versions (id, plan_id, version_no, ast, created_at)
            values ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(version.id.0)
        .bind(version.plan_id.0)
        .bind(version.version_no)
        .bind(&version.ast)
        .bind(version.created_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(version)
    }

    pub async fn list_by_plan(
        &self,
        tenant_id: TenantId,
        plan_id: ReportPlanId,
    ) -> Result<Vec<ReportPlanAstVersion>> {
        let rows = sqlx::query(
            r#"
            select v.id, v.plan_id, v.version_no, v.ast, v.created_at
            from report_plan_ast_versions v
            join report_plans p on p.id = v.plan_id
            where p.tenant_id = $1 and v.plan_id = $2
            order by v.version_no desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_report_plan_ast_version_row).collect()
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        ast_version_id: ReportPlanAstVersionId,
    ) -> Result<Option<ReportPlanAstVersion>> {
        let row = sqlx::query(
            r#"
            select v.id, v.plan_id, v.version_no, v.ast, v.created_at
            from report_plan_ast_versions v
            join report_plans p on p.id = v.plan_id
            where p.tenant_id = $1 and v.id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(ast_version_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref()
            .map(map_report_plan_ast_version_row)
            .transpose()
    }
}

#[derive(Clone)]
pub struct PgReportRenderOutputRepository {
    pool: PgPool,
}

impl PgReportRenderOutputRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_output: &NewReportRenderOutput,
    ) -> Result<ReportRenderOutput> {
        let row = sqlx::query(
            r#"
            insert into report_render_outputs (
                id,
                tenant_id,
                execution_id,
                plan_id,
                dataset_id,
                ast_version_id,
                surface,
                status,
                asset_manifest,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            returning id, tenant_id, execution_id, plan_id, dataset_id, ast_version_id, surface, status, asset_manifest, created_at
            "#,
        )
        .bind(ReportRenderOutputId::new().0)
        .bind(tenant_id.0)
        .bind(new_output.execution_id.0)
        .bind(new_output.plan_id.0)
        .bind(new_output.dataset_id.0)
        .bind(new_output.ast_version_id.0)
        .bind(new_output.surface.as_str())
        .bind(new_output.status.as_str())
        .bind(&new_output.asset_manifest)
        .bind(new_output.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_report_render_output_row(&row)
    }

    pub async fn list_by_plan(
        &self,
        tenant_id: TenantId,
        plan_id: ReportPlanId,
    ) -> Result<Vec<ReportRenderOutput>> {
        let rows = sqlx::query(
            r#"
            select o.id, o.tenant_id, o.execution_id, o.plan_id, o.dataset_id, o.ast_version_id,
                   o.surface, o.status, o.asset_manifest, o.created_at
            from report_render_outputs o
            join report_plans p on p.id = o.plan_id
            where p.tenant_id = $1 and o.plan_id = $2
            order by o.created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_report_render_output_row).collect()
    }

    pub async fn get_by_execution_id(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
    ) -> Result<Option<ReportRenderOutput>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, execution_id, plan_id, dataset_id, ast_version_id,
                   surface, status, asset_manifest, created_at
            from report_render_outputs
            where tenant_id = $1 and execution_id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(execution_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_report_render_output_row).transpose()
    }

    pub async fn get_latest_rendered_for_surface(
        &self,
        tenant_id: TenantId,
        plan_id: ReportPlanId,
        surface: domain_model::PublishedSurface,
    ) -> Result<Option<ReportRenderOutput>> {
        let row = sqlx::query(
            r#"
            select o.id, o.tenant_id, o.execution_id, o.plan_id, o.dataset_id, o.ast_version_id,
                   o.surface, o.status, o.asset_manifest, o.created_at
            from report_render_outputs o
            join report_plans p on p.id = o.plan_id
            where p.tenant_id = $1
              and o.plan_id = $2
              and o.surface = $3
              and o.status = $4
            order by o.created_at desc
            limit 1
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .bind(surface.as_str())
        .bind(ReportRenderOutputStatus::Rendered.as_str())
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_report_render_output_row).transpose()
    }
}

#[derive(Clone)]
pub struct PgPublishedReportRepository {
    pool: PgPool,
}

impl PgPublishedReportRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_report: &NewPublishedReport,
    ) -> Result<PublishedReport> {
        let row = sqlx::query(
            r#"
            insert into published_reports (
                id,
                tenant_id,
                dataset_id,
                plan_id,
                slug,
                current_version_id,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, null, $6, $7)
            returning id, tenant_id, dataset_id, plan_id, slug, current_version_id, created_at, updated_at
            "#,
        )
        .bind(PublishedReportId::new().0)
        .bind(tenant_id.0)
        .bind(new_report.dataset_id.0)
        .bind(new_report.plan_id.0)
        .bind(&new_report.slug)
        .bind(new_report.created_at)
        .bind(new_report.updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_published_report_row(&row)
    }

    pub async fn get_by_plan(
        &self,
        tenant_id: TenantId,
        plan_id: ReportPlanId,
    ) -> Result<Option<PublishedReport>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, plan_id, slug, current_version_id, created_at, updated_at
            from published_reports
            where tenant_id = $1 and plan_id = $2
            order by created_at asc
            limit 1
            "#,
        )
        .bind(tenant_id.0)
        .bind(plan_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_published_report_row).transpose()
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        report_id: PublishedReportId,
    ) -> Result<Option<PublishedReport>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, plan_id, slug, current_version_id, created_at, updated_at
            from published_reports
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(report_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_published_report_row).transpose()
    }

    pub async fn list_by_tenant(&self, tenant_id: TenantId) -> Result<Vec<PublishedReport>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, plan_id, slug, current_version_id, created_at, updated_at
            from published_reports
            where tenant_id = $1
            order by updated_at desc, created_at desc, slug asc
            "#,
        )
        .bind(tenant_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_published_report_row).collect()
    }

    pub async fn set_current_version(
        &self,
        tenant_id: TenantId,
        report_id: PublishedReportId,
        version_id: PublishedReportVersionId,
        updated_at: DateTime<Utc>,
    ) -> Result<PublishedReport> {
        let row = sqlx::query(
            r#"
            update published_reports
            set current_version_id = $3,
                updated_at = $4
            where tenant_id = $1 and id = $2
            returning id, tenant_id, dataset_id, plan_id, slug, current_version_id, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(report_id.0)
        .bind(version_id.0)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_published_report_row(&row)
    }
}

#[derive(Clone)]
pub struct PgPublishedReportVersionRepository {
    pool: PgPool,
}

impl PgPublishedReportVersionRepository {
    pub async fn create_next_version(
        &self,
        tenant_id: TenantId,
        report_id: PublishedReportId,
        new_version: &NewPublishedReportVersion,
    ) -> Result<PublishedReportVersion> {
        let mut tx = self.pool.begin().await?;
        let next_version_no = sqlx::query_scalar::<_, i32>(
            r#"
            select coalesce(max(v.version_no), 0) + 1
            from published_report_versions v
            join published_reports r on r.id = v.report_id
            where r.tenant_id = $1 and v.report_id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(report_id.0)
        .fetch_one(&mut *tx)
        .await?;

        let version = PublishedReportVersion {
            id: PublishedReportVersionId::new(),
            report_id,
            version_no: next_version_no,
            surface: new_version.surface.clone(),
            asset_manifest: new_version.asset_manifest.clone(),
            created_at: new_version.created_at,
        };

        sqlx::query(
            r#"
            insert into published_report_versions (
                id,
                report_id,
                version_no,
                surface,
                asset_manifest,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(version.id.0)
        .bind(version.report_id.0)
        .bind(version.version_no)
        .bind(version.surface.as_str())
        .bind(&version.asset_manifest)
        .bind(version.created_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(version)
    }

    pub async fn list_by_report(
        &self,
        tenant_id: TenantId,
        report_id: PublishedReportId,
    ) -> Result<Vec<PublishedReportVersion>> {
        let rows = sqlx::query(
            r#"
            select v.id, v.report_id, v.version_no, v.surface, v.asset_manifest, v.created_at
            from published_report_versions v
            join published_reports r on r.id = v.report_id
            where r.tenant_id = $1 and v.report_id = $2
            order by v.version_no desc, v.created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(report_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_published_report_version_row).collect()
    }
}

#[derive(Clone)]
pub struct PgPublishedVideoPptVersionRepository {
    pool: PgPool,
}

impl PgPublishedVideoPptVersionRepository {
    pub async fn create_next_version(
        &self,
        tenant_id: TenantId,
        new_version: &NewPublishedVideoPptVersion,
    ) -> Result<(PublishedVideoPptPackage, PublishedVideoPptVersion)> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            insert into published_video_ppt_packages (
                id,
                tenant_id,
                assistant_run_id,
                document_id,
                dataset_id,
                package_key,
                current_version_id,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, null, $7, $8)
            on conflict (tenant_id, assistant_run_id, document_id, package_key)
            do update set
                dataset_id = excluded.dataset_id,
                updated_at = excluded.updated_at
            returning id, tenant_id, assistant_run_id, document_id, dataset_id, package_key,
                      current_version_id, created_at, updated_at
            "#,
        )
        .bind(PublishedVideoPptPackageId::new().0)
        .bind(tenant_id.0)
        .bind(new_version.assistant_run_id.0)
        .bind(new_version.document_id.0)
        .bind(new_version.dataset_id.0)
        .bind(&new_version.package_key)
        .bind(new_version.created_at)
        .bind(new_version.created_at)
        .fetch_one(&mut *tx)
        .await?;
        let mut package = map_published_video_ppt_package_row(&row)?;

        if let Some(row) = sqlx::query(
            r#"
            select id, package_id, version_no, version_fingerprint, lifecycle_state, artifact_manifest, created_at
            from published_video_ppt_versions
            where package_id = $1 and version_fingerprint = $2
            "#,
        )
        .bind(package.id.0)
        .bind(&new_version.version_fingerprint)
        .fetch_optional(&mut *tx)
        .await?
        {
            let version = map_published_video_ppt_version_row(&row)?;
            let row = sqlx::query(
                r#"
                update published_video_ppt_packages
                set current_version_id = $3,
                    updated_at = $4
                where tenant_id = $1 and id = $2
                returning id, tenant_id, assistant_run_id, document_id, dataset_id, package_key,
                          current_version_id, created_at, updated_at
                "#,
            )
            .bind(tenant_id.0)
            .bind(package.id.0)
            .bind(version.id.0)
            .bind(new_version.created_at)
            .fetch_one(&mut *tx)
            .await?;
            package = map_published_video_ppt_package_row(&row)?;
            tx.commit().await?;
            return Ok((package, version));
        }

        let next_version_no = sqlx::query_scalar::<_, i32>(
            r#"
            select coalesce(max(version_no), 0) + 1
            from published_video_ppt_versions
            where package_id = $1
            "#,
        )
        .bind(package.id.0)
        .fetch_one(&mut *tx)
        .await?;

        let version = PublishedVideoPptVersion {
            id: PublishedVideoPptVersionId::new(),
            package_id: package.id,
            version_no: next_version_no,
            version_fingerprint: new_version.version_fingerprint.clone(),
            lifecycle_state: new_version.lifecycle_state.clone(),
            artifact_manifest: new_version.artifact_manifest.clone(),
            created_at: new_version.created_at,
        };

        sqlx::query(
            r#"
            insert into published_video_ppt_versions (
                id,
                package_id,
                version_no,
                version_fingerprint,
                lifecycle_state,
                artifact_manifest,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(version.id.0)
        .bind(version.package_id.0)
        .bind(version.version_no)
        .bind(&version.version_fingerprint)
        .bind(&version.lifecycle_state)
        .bind(&version.artifact_manifest)
        .bind(version.created_at)
        .execute(&mut *tx)
        .await?;

        let row = sqlx::query(
            r#"
            update published_video_ppt_packages
            set current_version_id = $3,
                updated_at = $4
            where tenant_id = $1 and id = $2
            returning id, tenant_id, assistant_run_id, document_id, dataset_id, package_key,
                      current_version_id, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(package.id.0)
        .bind(version.id.0)
        .bind(new_version.created_at)
        .fetch_one(&mut *tx)
        .await?;
        package = map_published_video_ppt_package_row(&row)?;

        tx.commit().await?;
        Ok((package, version))
    }

    pub async fn list_by_package_key(
        &self,
        tenant_id: TenantId,
        assistant_run_id: AssistantRunId,
        document_id: DocumentId,
        package_key: &str,
    ) -> Result<Vec<PublishedVideoPptVersion>> {
        let rows = sqlx::query(
            r#"
            select v.id, v.package_id, v.version_no, v.version_fingerprint, v.lifecycle_state,
                   v.artifact_manifest, v.created_at
            from published_video_ppt_versions v
            join published_video_ppt_packages p on p.id = v.package_id
            where p.tenant_id = $1
              and p.assistant_run_id = $2
              and p.document_id = $3
              and p.package_key = $4
            order by v.version_no desc, v.created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(assistant_run_id.0)
        .bind(document_id.0)
        .bind(package_key)
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(map_published_video_ppt_version_row)
            .collect()
    }
}

#[derive(Clone)]
pub struct PgMemoryDirectoryRepository {
    pool: PgPool,
}

impl PgMemoryDirectoryRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_directory: &NewMemoryDirectory,
    ) -> Result<MemoryDirectory> {
        let row = sqlx::query(
            r#"
            with next_version as (
                select coalesce(max(version_no), 0) + 1 as version_no
                from memory_directories
                where tenant_id = $2 and dataset_id = $3
            )
            insert into memory_directories (
                id,
                tenant_id,
                dataset_id,
                execution_id,
                owner_user_id,
                source_document_ids,
                version_no,
                directory_nodes,
                refreshed_chunks,
                directory_manifest,
                created_at
            )
            select
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                next_version.version_no,
                $7,
                $8,
                jsonb_set(
                    jsonb_set(
                        jsonb_set(
                            jsonb_set($9, '{version_no}', to_jsonb(next_version.version_no), true),
                            '{root,version_no}',
                            to_jsonb(next_version.version_no),
                            true
                        ),
                        '{owner_user_id}',
                        coalesce(to_jsonb($5::uuid), 'null'::jsonb),
                        true
                    ),
                    '{source_document_ids}',
                    to_jsonb($6::uuid[]),
                    true
                ),
                $10
            from next_version
            returning id, tenant_id, dataset_id, execution_id, owner_user_id, source_document_ids,
                      version_no, directory_nodes, refreshed_chunks, directory_manifest, created_at
            "#,
        )
        .bind(MemoryDirectoryId::new().0)
        .bind(tenant_id.0)
        .bind(new_directory.dataset_id.0)
        .bind(new_directory.execution_id.0)
        .bind(new_directory.owner_user_id.map(|id| id.0))
        .bind(document_ids_to_uuid_array(
            &new_directory.source_document_ids,
        ))
        .bind(new_directory.directory_nodes)
        .bind(new_directory.refreshed_chunks)
        .bind(&new_directory.directory_manifest)
        .bind(new_directory.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_memory_directory_row(&row)
    }

    pub async fn list_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Vec<MemoryDirectory>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, execution_id, owner_user_id, source_document_ids,
                   version_no, directory_nodes, refreshed_chunks, directory_manifest, created_at
            from memory_directories
            where tenant_id = $1 and dataset_id = $2
            order by version_no desc, created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_memory_directory_row).collect()
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        memory_directory_id: MemoryDirectoryId,
    ) -> Result<Option<MemoryDirectory>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, execution_id, owner_user_id, source_document_ids,
                   version_no, directory_nodes, refreshed_chunks, directory_manifest, created_at
            from memory_directories
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(memory_directory_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_memory_directory_row).transpose()
    }
}

#[derive(Clone)]
pub struct PgAssetRetrievalEvidenceRepository {
    pool: PgPool,
}

const ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL: &str = r#"
insert into asset_retrieval_evidences (
    id,
    tenant_id,
    dataset_id,
    asset_id,
    asset_profile_id,
    profile_kind,
    profile_version,
    materialized_text,
    safe_metadata,
    content_hash,
    search_terms,
    created_at,
    updated_at
)
select $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12
from asset_profiles profile
join asset_items asset
  on asset.id = profile.asset_id
 and asset.tenant_id = profile.tenant_id
join dataset_asset_memberships membership
  on membership.tenant_id = profile.tenant_id
 and membership.dataset_id = $3
 and membership.asset_id = profile.asset_id
where profile.id = $5
  and profile.tenant_id = $2
  and profile.asset_id = $4
  and profile.profile_kind = $6
  and profile.profile_version = $7
  and (membership.expires_at is null or membership.expires_at > $12)
  and exists (
      select 1
      from asset_parse_runs parse_run
      where parse_run.tenant_id = profile.tenant_id
        and parse_run.asset_id = profile.asset_id
        and parse_run.status in ('completed', 'partial')
        and concat(parse_run.parser_name, '@', parse_run.parser_version) = profile.profile_version
  )
on conflict (
    tenant_id,
    dataset_id,
    asset_id,
    profile_kind,
    profile_version,
    content_hash
) do nothing
returning id, tenant_id, dataset_id, asset_id, asset_profile_id, profile_kind,
          profile_version, materialized_text, safe_metadata, content_hash, search_terms,
          created_at, updated_at
"#;

impl PgAssetRetrievalEvidenceRepository {
    pub async fn create_if_eligible(
        &self,
        tenant_id: TenantId,
        evidence: NewAssetRetrievalEvidence,
    ) -> Result<Option<AssetRetrievalEvidenceRecord>> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL)
            .bind(Uuid::new_v4())
            .bind(tenant_id.0)
            .bind(evidence.dataset_id.0)
            .bind(evidence.asset_id)
            .bind(evidence.asset_profile_id)
            .bind(&evidence.profile_kind)
            .bind(&evidence.profile_version)
            .bind(&evidence.materialized_text)
            .bind(&evidence.safe_metadata)
            .bind(&evidence.content_hash)
            .bind(&evidence.search_terms)
            .bind(evidence.created_at)
            .fetch_optional(&mut *tx)
            .await?;
        let row = match row {
            Some(row) => Some(row),
            None => {
                sqlx::query(
                    r#"
                    select evidence.id, evidence.tenant_id, evidence.dataset_id,
                           evidence.asset_id, evidence.asset_profile_id,
                           evidence.profile_kind, evidence.profile_version,
                           evidence.materialized_text, evidence.safe_metadata,
                           evidence.content_hash, evidence.search_terms,
                           evidence.created_at, evidence.updated_at
                    from asset_retrieval_evidences evidence
                    join dataset_asset_memberships membership
                      on membership.tenant_id = evidence.tenant_id
                     and membership.dataset_id = evidence.dataset_id
                     and membership.asset_id = evidence.asset_id
                    where evidence.tenant_id = $1
                      and evidence.dataset_id = $2
                      and evidence.asset_id = $3
                      and evidence.profile_kind = $4
                      and evidence.profile_version = $5
                      and evidence.content_hash = $6
                      and (membership.expires_at is null or membership.expires_at > $7)
                    "#,
                )
                .bind(tenant_id.0)
                .bind(evidence.dataset_id.0)
                .bind(evidence.asset_id)
                .bind(&evidence.profile_kind)
                .bind(&evidence.profile_version)
                .bind(&evidence.content_hash)
                .bind(evidence.created_at)
                .fetch_optional(&mut *tx)
                .await?
            }
        };
        tx.commit().await?;
        Ok(row.as_ref().map(map_asset_retrieval_evidence_row))
    }

    pub async fn list_latest_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        limit: usize,
    ) -> Result<Vec<AssetRetrievalEvidenceRecord>> {
        let rows = sqlx::query(
            r#"
            select evidence.id, evidence.tenant_id, evidence.dataset_id,
                   evidence.asset_id, evidence.asset_profile_id,
                   evidence.profile_kind, evidence.profile_version,
                   evidence.materialized_text, evidence.safe_metadata,
                   evidence.content_hash, evidence.search_terms,
                   evidence.created_at, evidence.updated_at
            from asset_retrieval_evidences evidence
            join dataset_asset_memberships membership
              on membership.tenant_id = evidence.tenant_id
             and membership.dataset_id = evidence.dataset_id
             and membership.asset_id = evidence.asset_id
            where evidence.tenant_id = $1
              and evidence.dataset_id = $2
              and (membership.expires_at is null or membership.expires_at > now())
            order by evidence.updated_at desc, evidence.asset_id asc,
                     evidence.profile_kind asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(limit.clamp(1, 10_000) as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(map_asset_retrieval_evidence_row).collect())
    }

    pub async fn search(
        &self,
        query: AssetRetrievalEvidenceSearchQuery,
    ) -> Result<Vec<AssetRetrievalEvidenceRecord>> {
        let query_text = query.query.trim();
        if query_text.is_empty() || query.limit == 0 {
            return Ok(Vec::new());
        }
        let terms = retrieval_lexical_query_terms(query_text);
        let rows = sqlx::query(
            r#"
            select evidence.id, evidence.tenant_id, evidence.dataset_id,
                   evidence.asset_id, evidence.asset_profile_id,
                   evidence.profile_kind, evidence.profile_version,
                   evidence.materialized_text, evidence.safe_metadata,
                   evidence.content_hash, evidence.search_terms,
                   evidence.created_at, evidence.updated_at
            from asset_retrieval_evidences evidence
            join dataset_asset_memberships membership
              on membership.tenant_id = evidence.tenant_id
             and membership.dataset_id = evidence.dataset_id
             and membership.asset_id = evidence.asset_id
            where evidence.tenant_id = $1
              and evidence.dataset_id = $2
              and (membership.expires_at is null or membership.expires_at > now())
              and (
                  evidence.search_tsv @@ plainto_tsquery('simple', $3)
                  or evidence.search_terms && $4::text[]
                  or position(lower($3) in lower(evidence.materialized_text)) > 0
              )
            order by
              case when position(lower($3) in lower(evidence.materialized_text)) > 0 then 1 else 0 end desc,
              cardinality(array(select unnest(evidence.search_terms) intersect select unnest($4::text[]))) desc,
              ts_rank_cd(evidence.search_tsv, plainto_tsquery('simple', $3)) desc,
              evidence.updated_at desc,
              evidence.asset_id asc,
              evidence.profile_kind asc
            limit $5
            "#,
        )
        .bind(query.tenant_id.0)
        .bind(query.dataset_id.0)
        .bind(query_text)
        .bind(terms)
        .bind(query.limit.min(100) as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(map_asset_retrieval_evidence_row).collect())
    }
}

#[derive(Clone)]
pub struct PgRetrievalEvidenceRepository {
    pool: PgPool,
}

const RETRIEVAL_EVIDENCE_UPSERT_SQL: &str = r#"
insert into retrieval_evidences (
    id,
    tenant_id,
    dataset_id,
    execution_id,
    document_id,
    document_chunk_id,
    chunk_index,
    source_locator,
    content_excerpt,
    summary,
    payload_filter_key,
    embedding_model,
    recall_score,
    evidence_manifest,
    search_text,
    search_terms,
    search_tsv,
    search_language,
    indexed_content_hash,
    indexed_at,
    created_at
)
values (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
    coalesce(nullif($14 #>> '{lexical,search_text}', ''), concat_ws(E'\n', $10, $9, $8, $11)),
    case
        when jsonb_typeof($14 #> '{lexical,search_terms}') = 'array'
            then $14 #> '{lexical,search_terms}'
        else '[]'::jsonb
    end,
    to_tsvector(
        'simple',
        coalesce(nullif($14 #>> '{lexical,search_text}', ''), concat_ws(E'\n', $10, $9, $8, $11))
    ),
    coalesce(nullif($14 #>> '{lexical,language}', ''), 'simple'),
    coalesce(nullif($14 #>> '{lexical,indexed_content_hash}', ''), md5(concat_ws(E'\n', $10, $9, $8, $11))),
    coalesce((nullif($14 #>> '{lexical,indexed_at}', ''))::timestamptz, $15),
    $15
)
on conflict (execution_id, document_chunk_id) do update
set dataset_id = excluded.dataset_id,
    document_id = excluded.document_id,
    chunk_index = excluded.chunk_index,
    source_locator = excluded.source_locator,
    content_excerpt = excluded.content_excerpt,
    summary = excluded.summary,
    payload_filter_key = excluded.payload_filter_key,
    embedding_model = excluded.embedding_model,
    recall_score = excluded.recall_score,
    evidence_manifest = excluded.evidence_manifest,
    search_text = excluded.search_text,
    search_terms = excluded.search_terms,
    search_tsv = excluded.search_tsv,
    search_language = excluded.search_language,
    indexed_content_hash = excluded.indexed_content_hash,
    indexed_at = excluded.indexed_at,
    created_at = excluded.created_at
where retrieval_evidences.tenant_id = excluded.tenant_id
returning id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
          chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
          embedding_model, recall_score, evidence_manifest, created_at
"#;

impl PgRetrievalEvidenceRepository {
    pub async fn create_many(
        &self,
        tenant_id: TenantId,
        evidences: &[NewRetrievalEvidence],
    ) -> Result<Vec<RetrievalEvidence>> {
        let mut tx = self.pool.begin().await?;
        let mut persisted = Vec::with_capacity(evidences.len());

        for evidence in evidences {
            let row = sqlx::query(RETRIEVAL_EVIDENCE_UPSERT_SQL)
                .bind(RetrievalEvidenceId::new().0)
                .bind(tenant_id.0)
                .bind(evidence.dataset_id.0)
                .bind(evidence.execution_id.0)
                .bind(evidence.document_id.0)
                .bind(evidence.document_chunk_id.0)
                .bind(evidence.chunk_index)
                .bind(&evidence.source_locator)
                .bind(&evidence.content_excerpt)
                .bind(&evidence.summary)
                .bind(&evidence.payload_filter_key)
                .bind(&evidence.embedding_model)
                .bind(evidence.recall_score)
                .bind(&evidence.evidence_manifest)
                .bind(evidence.created_at)
                .fetch_one(&mut *tx)
                .await?;

            persisted.push(map_retrieval_evidence_row(&row)?);
        }

        tx.commit().await?;
        Ok(persisted)
    }

    pub async fn list_by_document(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
    ) -> Result<Vec<RetrievalEvidence>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                   chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                   embedding_model, recall_score, evidence_manifest, created_at
            from retrieval_evidences
            where tenant_id = $1 and document_id = $2
            order by created_at desc, chunk_index asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_retrieval_evidence_row).collect()
    }

    pub async fn list_by_document_or_canonical(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
    ) -> Result<Vec<RetrievalEvidence>> {
        let effective_document_id =
            resolve_canonical_document_id(&self.pool, tenant_id, document_id).await?;
        self.list_by_document(tenant_id, effective_document_id)
            .await
    }

    pub async fn list_latest_by_document_ids(
        &self,
        tenant_id: TenantId,
        document_ids: &[DocumentId],
        limit: usize,
    ) -> Result<Vec<RetrievalEvidence>> {
        if document_ids.is_empty() {
            return Ok(Vec::new());
        }
        let document_ids = document_ids.iter().map(|id| id.0).collect::<Vec<_>>();
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                   chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                   embedding_model, recall_score, evidence_manifest, created_at
            from (
                select distinct on (document_chunk_id)
                       id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                       chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                       embedding_model, recall_score, evidence_manifest, created_at
                from retrieval_evidences
                where tenant_id = $1 and document_id = any($2)
                order by document_chunk_id, created_at desc
            ) latest
            order by created_at desc, document_id asc, chunk_index asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(document_ids)
        .bind(limit.clamp(1, 50_000) as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_retrieval_evidence_row).collect()
    }

    pub async fn list_latest_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        limit: i64,
    ) -> Result<Vec<RetrievalEvidence>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                   chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                   embedding_model, recall_score, evidence_manifest, created_at
            from (
                select distinct on (document_chunk_id)
                    id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                    chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                    embedding_model, recall_score, evidence_manifest, created_at
                from retrieval_evidences
                where tenant_id = $1
                  and document_id in (
                    select coalesce(canonical_document_id, id)
                    from documents
                    where tenant_id = $1 and dataset_id = $2
                  )
                order by document_chunk_id, created_at desc
            ) latest
            order by created_at desc, document_id asc, chunk_index asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_retrieval_evidence_row).collect()
    }

    pub async fn list_latest_by_dataset_scope(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        limit: i64,
    ) -> Result<Vec<RetrievalEvidence>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                   chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                   embedding_model, recall_score, evidence_manifest, created_at
            from (
                select distinct on (document_chunk_id)
                    id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                    chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                    embedding_model, recall_score, evidence_manifest, created_at
                from retrieval_evidences
                where tenant_id = $1
                  and document_id in (
                    select coalesce(d.canonical_document_id, d.id)
                    from documents d
                    where d.tenant_id = $1
                      and (
                        d.dataset_id = $2
                        or d.id in (
                            select document_id
                            from dataset_document_memberships
                            where tenant_id = $1
                              and dataset_id = $2
                              and (expires_at is null or expires_at > now())
                        )
                      )
                  )
                order by document_chunk_id, created_at desc
            ) latest
            order by created_at desc, document_id asc, chunk_index asc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_retrieval_evidence_row).collect()
    }

    pub async fn search_lexical_retrieval_evidences(
        &self,
        query: LexicalRetrievalQuery,
    ) -> Result<Vec<RetrievalEvidence>> {
        let terms = retrieval_lexical_query_terms(&query.query);
        let document_ids = document_ids_to_uuid_array(&query.document_ids);
        let owner_user_id = query.owner_user_id.map(|id| id.0);
        let candidate_limit = query.candidate_limit.max(query.limit).max(1) as i64;
        let query_text = query.query.trim();
        if query_text.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            with scoped_documents as (
                select coalesce(d.canonical_document_id, d.id) as document_id
                from documents d
                where d.tenant_id = $1
                  and (
                    d.dataset_id = $2
                    or d.id in (
                        select document_id
                        from dataset_document_memberships
                        where tenant_id = $1
                          and dataset_id = $2
                          and (expires_at is null or expires_at > now())
                    )
                  )
                  and (
                    cardinality($3::uuid[]) = 0
                    or d.id = any($3::uuid[])
                    or coalesce(d.canonical_document_id, d.id) = any($3::uuid[])
                  )
                  and (d.owner_user_id is null or d.owner_user_id = $4)
            ),
            candidates as (
                select ev.id, ev.tenant_id, ev.dataset_id, ev.execution_id, ev.document_id,
                       ev.document_chunk_id, ev.chunk_index, ev.source_locator,
                       ev.content_excerpt, ev.summary, ev.payload_filter_key,
                       ev.embedding_model, ev.recall_score, ev.evidence_manifest,
                       ev.created_at,
                       coalesce(
                           ev.search_text,
                           concat_ws(E'\n', ev.summary, ev.content_excerpt, ev.source_locator, ev.payload_filter_key)
                       ) as lexical_text,
                       case
                         when jsonb_typeof(coalesce(ev.search_terms, '[]'::jsonb)) = 'array'
                           then coalesce(ev.search_terms, '[]'::jsonb)
                         else '[]'::jsonb
                       end as lexical_terms,
                       coalesce(
                           ev.search_tsv,
                           to_tsvector(
                               'simple',
                               concat_ws(E'\n', ev.summary, ev.content_excerpt, ev.source_locator, ev.payload_filter_key)
                           )
                       ) as lexical_tsv
                from retrieval_evidences ev
                join scoped_documents sd on sd.document_id = ev.document_id
                where ev.tenant_id = $1
            ),
            ranked as (
                select candidates.*,
                       ts_rank_cd(candidates.lexical_tsv, plainto_tsquery('simple', $5)) as tsv_score,
                       (
                         select count(*)::int
                         from jsonb_array_elements_text(candidates.lexical_terms) as term(value)
                         where term.value = any($6::text[])
                       ) as term_hits,
                       case
                         when position(lower($5) in lower(candidates.lexical_text)) > 0 then 1
                         else 0
                       end as phrase_hit
                from candidates
                where candidates.lexical_tsv @@ plainto_tsquery('simple', $5)
                   or candidates.lexical_terms ?| $6::text[]
                   or position(lower($5) in lower(candidates.lexical_text)) > 0
            ),
            deduped as (
                select distinct on (document_chunk_id)
                       id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                       chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                       embedding_model, recall_score, evidence_manifest, created_at,
                       phrase_hit, term_hits, tsv_score
                from ranked
                order by document_chunk_id, phrase_hit desc, term_hits desc, tsv_score desc, created_at desc
            )
            select id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                   chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                   embedding_model, recall_score, evidence_manifest, created_at
            from deduped
            order by phrase_hit desc, term_hits desc, tsv_score desc,
                     recall_score desc, created_at desc, document_id asc, chunk_index asc
            limit $7
            "#,
        )
        .bind(query.tenant_id.0)
        .bind(query.dataset_id.0)
        .bind(document_ids)
        .bind(owner_user_id)
        .bind(query_text)
        .bind(terms)
        .bind(candidate_limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_retrieval_evidence_row).collect()
    }

    pub async fn list_by_ids(
        &self,
        tenant_id: TenantId,
        evidence_ids: &[RetrievalEvidenceId],
    ) -> Result<Vec<RetrievalEvidence>> {
        if evidence_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                   chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                   embedding_model, recall_score, evidence_manifest, created_at
            from retrieval_evidences
            where tenant_id = $1 and id = any($2)
            "#,
        )
        .bind(tenant_id.0)
        .bind(retrieval_evidence_ids_to_uuid_array(evidence_ids))
        .fetch_all(&self.pool)
        .await?;

        let evidences = rows
            .iter()
            .map(map_retrieval_evidence_row)
            .collect::<Result<Vec<_>>>()?;
        let by_id = std::collections::BTreeMap::from_iter(
            evidences
                .into_iter()
                .map(|evidence| (evidence.id, evidence)),
        );

        Ok(evidence_ids
            .iter()
            .filter_map(|id| by_id.get(id).cloned())
            .collect())
    }
}

#[derive(Clone)]
pub struct PgDatasetOutputRepository {
    pool: PgPool,
}

impl PgDatasetOutputRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_output: &NewDatasetOutput,
    ) -> Result<DatasetOutput> {
        let row = sqlx::query(
            r#"
            insert into dataset_outputs (
                id,
                tenant_id,
                execution_id,
                dataset_id,
                owner_user_id,
                prompt,
                output_text,
                memory_directory_id,
                retrieval_evidence_ids,
                output_manifest,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            returning id, tenant_id, execution_id, dataset_id, owner_user_id, prompt, output_text,
                      memory_directory_id, retrieval_evidence_ids, output_manifest, created_at
            "#,
        )
        .bind(DatasetOutputId::new().0)
        .bind(tenant_id.0)
        .bind(new_output.execution_id.0)
        .bind(new_output.dataset_id.0)
        .bind(new_output.owner_user_id.map(|id| id.0))
        .bind(&new_output.prompt)
        .bind(&new_output.output_text)
        .bind(new_output.memory_directory_id.map(|value| value.0))
        .bind(retrieval_evidence_ids_to_uuid_array(
            &new_output.retrieval_evidence_ids,
        ))
        .bind(&new_output.output_manifest)
        .bind(new_output.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_dataset_output_row(&row)
    }

    pub async fn list_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Vec<DatasetOutput>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, dataset_id, prompt, output_text,
                   owner_user_id, memory_directory_id, retrieval_evidence_ids, output_manifest, created_at
            from dataset_outputs
            where tenant_id = $1 and dataset_id = $2
            order by created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_dataset_output_row).collect()
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        dataset_output_id: DatasetOutputId,
    ) -> Result<Option<DatasetOutput>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, execution_id, dataset_id, prompt, output_text,
                   owner_user_id, memory_directory_id, retrieval_evidence_ids, output_manifest, created_at
            from dataset_outputs
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_output_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_dataset_output_row).transpose()
    }

    pub async fn get_by_execution_id(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
    ) -> Result<Option<DatasetOutput>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, execution_id, dataset_id, prompt, output_text,
                   owner_user_id, memory_directory_id, retrieval_evidence_ids, output_manifest, created_at
            from dataset_outputs
            where tenant_id = $1 and execution_id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(execution_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_dataset_output_row).transpose()
    }
}

#[derive(Clone)]
pub struct PgAssistantRunRepository {
    pool: PgPool,
}

impl PgAssistantRunRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_run: &NewAssistantRun,
    ) -> Result<AssistantRun> {
        let row = sqlx::query(
            r#"
            insert into assistant_runs (
                tenant_id,
                user_id,
                local_thread_id,
                user_prompt,
                startup_briefing,
                selected_scope,
                scope_candidates,
                context_policy,
                evidence_state,
                service_lane,
                execution_trail,
                output_artifacts,
                runtime_manifest,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)
            returning id, tenant_id, user_id, local_thread_id, user_prompt, startup_briefing,
                      selected_scope, scope_candidates, context_policy, evidence_state,
                      service_lane, execution_trail, output_artifacts, runtime_manifest,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_run.user_id.map(|id| id.0))
        .bind(new_run.local_thread_id.as_deref())
        .bind(&new_run.user_prompt)
        .bind(&new_run.startup_briefing)
        .bind(&new_run.selected_scope)
        .bind(&new_run.scope_candidates)
        .bind(&new_run.context_policy)
        .bind(&new_run.evidence_state)
        .bind(&new_run.service_lane)
        .bind(&new_run.execution_trail)
        .bind(&new_run.output_artifacts)
        .bind(&new_run.runtime_manifest)
        .bind(new_run.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_assistant_run_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
    ) -> Result<Option<AssistantRun>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, user_id, local_thread_id, user_prompt, startup_briefing,
                   selected_scope, scope_candidates, context_policy, evidence_state,
                   service_lane, execution_trail, output_artifacts, runtime_manifest,
                   created_at, updated_at
            from assistant_runs
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(run_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_assistant_run_row).transpose()
    }

    pub async fn list_by_local_thread(
        &self,
        tenant_id: TenantId,
        local_thread_id: &str,
        limit: i64,
    ) -> Result<Vec<AssistantRun>> {
        let normalized_limit = limit.clamp(1, 100);
        let rows = sqlx::query(
            r#"
            select id, tenant_id, user_id, local_thread_id, user_prompt, startup_briefing,
                   selected_scope, scope_candidates, context_policy, evidence_state,
                   service_lane, execution_trail, output_artifacts, runtime_manifest,
                   created_at, updated_at
            from assistant_runs
            where tenant_id = $1 and local_thread_id = $2
            order by updated_at desc, created_at desc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(local_thread_id)
        .bind(normalized_limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_assistant_run_row).collect()
    }

    pub async fn append_event(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
        new_event: &NewAssistantRunEvent,
    ) -> Result<AssistantRunEvent> {
        let mut attempt = 0u64;
        loop {
            let result = sqlx::query(
                r#"
                with run_event_lock as (
                    select pg_advisory_xact_lock(hashtext($1::text), hashtext($2::text))
                ),
                next_sequence as (
                    select coalesce((
                        select max(sequence_no)
                        from assistant_run_events
                        where tenant_id = $1 and run_id = $2
                    ), 0) + 1 as sequence_no
                    from run_event_lock
                )
                insert into assistant_run_events (
                    tenant_id,
                    run_id,
                    sequence_no,
                    event_name,
                    payload,
                    created_at
                )
                select $1, $2, next_sequence.sequence_no, $3, $4, $5
                from next_sequence
                returning id, tenant_id, run_id, sequence_no, event_name, payload, created_at
                "#,
            )
            .bind(tenant_id.0)
            .bind(run_id.0)
            .bind(&new_event.event_name)
            .bind(&new_event.payload)
            .bind(new_event.created_at)
            .fetch_one(&self.pool)
            .await;

            match result {
                Ok(row) => return map_assistant_run_event_row(&row),
                Err(error)
                    if attempt < 5 && is_assistant_run_event_sequence_unique_violation(&error) =>
                {
                    attempt += 1;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub async fn list_events(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
    ) -> Result<Vec<AssistantRunEvent>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, run_id, sequence_no, event_name, payload, created_at
            from assistant_run_events
            where tenant_id = $1 and run_id = $2
            order by sequence_no asc, created_at asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(run_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_assistant_run_event_row).collect()
    }

    pub async fn update_selected_scope(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
        selected_scope: &Value,
    ) -> Result<AssistantRun> {
        self.update_json_field(tenant_id, run_id, "selected_scope", selected_scope)
            .await
    }

    pub async fn update_evidence_state(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
        evidence_state: &Value,
    ) -> Result<AssistantRun> {
        self.update_json_field(tenant_id, run_id, "evidence_state", evidence_state)
            .await
    }

    pub async fn attach_output_artifacts(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
        output_artifacts: &Value,
    ) -> Result<AssistantRun> {
        self.update_json_field(tenant_id, run_id, "output_artifacts", output_artifacts)
            .await
    }

    pub async fn update_execution_trail(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
        execution_trail: &Value,
    ) -> Result<AssistantRun> {
        self.update_json_field(tenant_id, run_id, "execution_trail", execution_trail)
            .await
    }

    pub async fn update_runtime_manifest(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
        runtime_manifest: &Value,
    ) -> Result<AssistantRun> {
        self.update_json_field(tenant_id, run_id, "runtime_manifest", runtime_manifest)
            .await
    }

    async fn update_json_field(
        &self,
        tenant_id: TenantId,
        run_id: AssistantRunId,
        field_name: &str,
        value: &Value,
    ) -> Result<AssistantRun> {
        let allowed = matches!(
            field_name,
            "selected_scope"
                | "evidence_state"
                | "execution_trail"
                | "output_artifacts"
                | "runtime_manifest"
        );
        if !allowed {
            return Err(anyhow!(
                "unsupported assistant run JSON field: {field_name}"
            ));
        }
        let sql = format!(
            r#"
            update assistant_runs
            set {field_name} = $3,
                updated_at = now()
            where tenant_id = $1 and id = $2
            returning id, tenant_id, user_id, local_thread_id, user_prompt, startup_briefing,
                      selected_scope, scope_candidates, context_policy, evidence_state,
                      service_lane, execution_trail, output_artifacts, runtime_manifest,
                      created_at, updated_at
            "#
        );
        let row = sqlx::query(AssertSqlSafe(sql))
            .bind(tenant_id.0)
            .bind(run_id.0)
            .bind(value)
            .fetch_one(&self.pool)
            .await?;

        map_assistant_run_row(&row)
    }
}

#[derive(Clone)]
pub struct PgConversationMemoryItemRepository {
    pool: PgPool,
}

impl PgConversationMemoryItemRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_item: &NewConversationMemoryItem,
    ) -> Result<ConversationMemoryItem> {
        let row = sqlx::query(
            r#"
            insert into conversation_memory_items (
                tenant_id,
                user_id,
                local_thread_id,
                role,
                item_kind,
                summary,
                source_message_refs,
                artifact_refs,
                metadata,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            returning id, tenant_id, user_id, local_thread_id, role, item_kind, summary,
                      source_message_refs, artifact_refs, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_item.user_id.map(|id| id.0))
        .bind(&new_item.local_thread_id)
        .bind(new_item.role.as_str())
        .bind(&new_item.item_kind)
        .bind(&new_item.summary)
        .bind(&new_item.source_message_refs)
        .bind(&new_item.artifact_refs)
        .bind(&new_item.metadata)
        .bind(new_item.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_conversation_memory_item_row(&row)
    }

    pub async fn list_by_local_thread(
        &self,
        tenant_id: TenantId,
        local_thread_id: &str,
        query: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ConversationMemoryItem>> {
        let normalized_limit = limit.clamp(1, 100);
        let rows = if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
            sqlx::query(
                r#"
                select id, tenant_id, user_id, local_thread_id, role, item_kind, summary,
                       source_message_refs, artifact_refs, metadata, created_at, updated_at
                from conversation_memory_items
                where tenant_id = $1
                  and local_thread_id = $2
                  and summary ilike $3
                order by updated_at desc, created_at desc
                limit $4
                "#,
            )
            .bind(tenant_id.0)
            .bind(local_thread_id)
            .bind(format!("%{query}%"))
            .bind(normalized_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                select id, tenant_id, user_id, local_thread_id, role, item_kind, summary,
                       source_message_refs, artifact_refs, metadata, created_at, updated_at
                from conversation_memory_items
                where tenant_id = $1 and local_thread_id = $2
                order by updated_at desc, created_at desc
                limit $3
                "#,
            )
            .bind(tenant_id.0)
            .bind(local_thread_id)
            .bind(normalized_limit)
            .fetch_all(&self.pool)
            .await?
        };

        rows.iter().map(map_conversation_memory_item_row).collect()
    }
}

#[derive(Clone)]
pub struct PgHtmlArtifactRepository {
    pool: PgPool,
}

impl PgHtmlArtifactRepository {
    pub async fn upsert(
        &self,
        tenant_id: TenantId,
        artifact: &NewHtmlArtifact,
    ) -> Result<HtmlArtifact> {
        let row = sqlx::query(
            r#"
            insert into html_artifacts (
                id,
                tenant_id,
                owner_user_id,
                assistant_run_id,
                local_thread_id,
                source_type,
                template_id,
                interaction_mode,
                manifest,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            on conflict (tenant_id, id) do update
            set owner_user_id = excluded.owner_user_id,
                assistant_run_id = excluded.assistant_run_id,
                local_thread_id = excluded.local_thread_id,
                source_type = excluded.source_type,
                template_id = excluded.template_id,
                interaction_mode = excluded.interaction_mode,
                manifest = excluded.manifest,
                updated_at = now()
            returning id, tenant_id, owner_user_id, assistant_run_id, local_thread_id,
                      source_type, template_id, interaction_mode, manifest, created_at, updated_at
            "#,
        )
        .bind(&artifact.id)
        .bind(tenant_id.0)
        .bind(artifact.owner_user_id.map(|id| id.0))
        .bind(artifact.assistant_run_id.map(|id| id.0))
        .bind(&artifact.local_thread_id)
        .bind(&artifact.source_type)
        .bind(&artifact.template_id)
        .bind(&artifact.interaction_mode)
        .bind(&artifact.manifest)
        .bind(artifact.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_html_artifact_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        artifact_id: &str,
    ) -> Result<Option<HtmlArtifact>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, assistant_run_id, local_thread_id,
                   source_type, template_id, interaction_mode, manifest, created_at, updated_at
            from html_artifacts
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(artifact_id)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_html_artifact_row).transpose()
    }

    pub async fn list_by_assistant_run(
        &self,
        tenant_id: TenantId,
        assistant_run_id: AssistantRunId,
        limit: i64,
    ) -> Result<Vec<HtmlArtifact>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, assistant_run_id, local_thread_id,
                   source_type, template_id, interaction_mode, manifest, created_at, updated_at
            from html_artifacts
            where tenant_id = $1 and assistant_run_id = $2
            order by updated_at desc, created_at desc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(assistant_run_id.0)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_html_artifact_row).collect()
    }

    pub async fn list_by_local_thread(
        &self,
        tenant_id: TenantId,
        local_thread_id: &str,
        limit: i64,
    ) -> Result<Vec<HtmlArtifact>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, assistant_run_id, local_thread_id,
                   source_type, template_id, interaction_mode, manifest, created_at, updated_at
            from html_artifacts
            where tenant_id = $1 and local_thread_id = $2
            order by updated_at desc, created_at desc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(local_thread_id)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_html_artifact_row).collect()
    }
}

#[derive(Clone)]
pub struct PgStaticPageDraftRepository {
    pool: PgPool,
}

impl PgStaticPageDraftRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_draft: &NewStaticPageDraft,
    ) -> Result<StaticPageDraft> {
        let row = sqlx::query(
            r#"
            insert into static_page_drafts (
                tenant_id,
                owner_user_id,
                assistant_run_id,
                title,
                status,
                selected_scope,
                visibility_snapshot,
                source_refs,
                draft_payload,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            returning id, tenant_id, owner_user_id, assistant_run_id, title, status, selected_scope,
                      visibility_snapshot, source_refs, draft_payload, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_draft.owner_user_id.map(|id| id.0))
        .bind(new_draft.assistant_run_id.0)
        .bind(&new_draft.title)
        .bind(new_draft.status.as_str())
        .bind(&new_draft.selected_scope)
        .bind(&new_draft.visibility_snapshot)
        .bind(&new_draft.source_refs)
        .bind(&new_draft.draft_payload)
        .bind(new_draft.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_static_page_draft_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        draft_id: StaticPageDraftId,
    ) -> Result<Option<StaticPageDraft>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, assistant_run_id, title, status, selected_scope,
                   visibility_snapshot, source_refs, draft_payload, created_at, updated_at
            from static_page_drafts
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(draft_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_static_page_draft_row).transpose()
    }

    pub async fn list_by_assistant_run(
        &self,
        tenant_id: TenantId,
        assistant_run_id: AssistantRunId,
    ) -> Result<Vec<StaticPageDraft>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, assistant_run_id, title, status, selected_scope,
                   visibility_snapshot, source_refs, draft_payload, created_at, updated_at
            from static_page_drafts
            where tenant_id = $1 and assistant_run_id = $2
            order by updated_at desc, created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(assistant_run_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_static_page_draft_row).collect()
    }

    pub async fn list_by_local_thread(
        &self,
        tenant_id: TenantId,
        local_thread_id: &str,
        limit: i64,
    ) -> Result<Vec<StaticPageDraft>> {
        let rows = sqlx::query(
            r#"
            select d.id, d.tenant_id, d.owner_user_id, d.assistant_run_id, d.title, d.status,
                   d.selected_scope, d.visibility_snapshot, d.source_refs,
                   d.draft_payload, d.created_at, d.updated_at
            from static_page_drafts d
            join assistant_runs r
              on r.id = d.assistant_run_id
             and r.tenant_id = d.tenant_id
            where d.tenant_id = $1
              and r.local_thread_id = $2
            order by d.updated_at desc, d.created_at desc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(local_thread_id)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_static_page_draft_row).collect()
    }

    pub async fn list_by_dataset_id(
        &self,
        tenant_id: TenantId,
        dataset_id: &str,
        limit: i64,
    ) -> Result<Vec<StaticPageDraft>> {
        let rows = sqlx::query(
            r#"
            select d.id, d.tenant_id, d.owner_user_id, d.assistant_run_id, d.title, d.status,
                   d.selected_scope, d.visibility_snapshot, d.source_refs,
                   d.draft_payload, d.created_at, d.updated_at
            from static_page_drafts d
            left join datasets ds
              on ds.tenant_id = d.tenant_id
             and ds.id::text = $2
            where d.tenant_id = $1
              and (
                    d.source_refs ->> 'dataset_id' = $2
                 or d.source_refs ->> 'datasetId' = $2
                 or d.draft_payload ->> 'datasetId' = $2
                 or d.draft_payload ->> 'dataset_id' = $2
                 or d.draft_payload #>> '{source,datasetId}' = $2
                 or d.draft_payload #>> '{source,dataset_id}' = $2
                 or position($2 in d.selected_scope::text) > 0
                 or (
                      ds.id is not null
                  and (
                         (nullif(ds.key, '') is not null and (
                              d.source_refs::text ilike '%' || ds.key || '%'
                           or d.draft_payload::text ilike '%' || ds.key || '%'
                         ))
                      or (nullif(ds.title, '') is not null and (
                              d.source_refs::text ilike '%' || ds.title || '%'
                           or d.draft_payload::text ilike '%' || ds.title || '%'
                         ))
                  )
                 )
              )
            order by d.updated_at desc, d.created_at desc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_static_page_draft_row).collect()
    }

    pub async fn find_latest_accepted_baseline_by_artifact_key(
        &self,
        tenant_id: TenantId,
        artifact_key: &str,
    ) -> Result<Option<StaticPageDraft>> {
        let artifact_key = artifact_key.trim();
        if artifact_key.is_empty() {
            return Ok(None);
        }
        let row = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, assistant_run_id, title, status, selected_scope,
                   visibility_snapshot, source_refs, draft_payload, created_at, updated_at
            from static_page_drafts
            where tenant_id = $1
              and (
                    source_refs #>> '{artifact_stability,dataset_artifact_key}' = $2
                 or source_refs ->> 'dataset_artifact_key' = $2
                 or draft_payload #>> '{artifactStability,datasetArtifactKey}' = $2
                 or draft_payload #>> '{artifact_stability,dataset_artifact_key}' = $2
              )
              and coalesce(
                    nullif(source_refs #>> '{artifact_stability,baseline_status}', ''),
                    nullif(source_refs #>> '{artifact_stability,baselineStatus}', ''),
                    nullif(draft_payload #>> '{artifact_stability,baseline_status}', ''),
                    nullif(draft_payload #>> '{artifact_stability,baselineStatus}', ''),
                    nullif(draft_payload ->> 'baseline_status', ''),
                    nullif(draft_payload ->> 'baselineStatus', ''),
                    nullif(draft_payload #>> '{artifactStability,baselineStatus}', ''),
                    nullif(draft_payload #>> '{artifactStability,baseline_status}', ''),
                    nullif(draft_payload #>> '{finalPage,baselineStatus}', ''),
                    nullif(draft_payload #>> '{finalPage,baseline_status}', ''),
                    nullif(draft_payload #>> '{final_page,baseline_status}', ''),
                    nullif(draft_payload #>> '{final_page,baselineStatus}', '')
              ) = 'accepted'
            order by updated_at desc, created_at desc
            limit 1
            "#,
        )
        .bind(tenant_id.0)
        .bind(artifact_key)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_static_page_draft_row).transpose()
    }

    pub async fn find_accepted_baseline_by_public_url(
        &self,
        tenant_id: TenantId,
        public_url: &str,
    ) -> Result<Option<StaticPageDraft>> {
        let public_url = public_url.trim();
        if public_url.is_empty() {
            return Ok(None);
        }
        let row = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, assistant_run_id, title, status, selected_scope,
                   visibility_snapshot, source_refs, draft_payload, created_at, updated_at
            from static_page_drafts
            where tenant_id = $1
              and (
                    draft_payload #>> '{finalPage,publicUrl}' = $2
                 or draft_payload #>> '{finalPage,public_url}' = $2
                 or draft_payload #>> '{finalPage,generatedArtifactUrl}' = $2
                 or draft_payload #>> '{finalPage,generated_artifact_url}' = $2
                 or draft_payload #>> '{final_page,publicUrl}' = $2
                 or draft_payload #>> '{final_page,public_url}' = $2
                 or draft_payload #>> '{artifactStability,publicUrl}' = $2
                 or draft_payload #>> '{artifactStability,public_url}' = $2
                 or draft_payload #>> '{artifact_stability,public_url}' = $2
                 or source_refs #>> '{artifact_stability,public_url}' = $2
                 or source_refs #>> '{artifact_stability,publicUrl}' = $2
              )
              and coalesce(
                    nullif(source_refs #>> '{artifact_stability,baseline_status}', ''),
                    nullif(source_refs #>> '{artifact_stability,baselineStatus}', ''),
                    nullif(draft_payload #>> '{artifact_stability,baseline_status}', ''),
                    nullif(draft_payload #>> '{artifact_stability,baselineStatus}', ''),
                    nullif(draft_payload ->> 'baseline_status', ''),
                    nullif(draft_payload ->> 'baselineStatus', ''),
                    nullif(draft_payload #>> '{artifactStability,baselineStatus}', ''),
                    nullif(draft_payload #>> '{artifactStability,baseline_status}', ''),
                    nullif(draft_payload #>> '{finalPage,baselineStatus}', ''),
                    nullif(draft_payload #>> '{finalPage,baseline_status}', ''),
                    nullif(draft_payload #>> '{final_page,baseline_status}', ''),
                    nullif(draft_payload #>> '{final_page,baselineStatus}', '')
              ) = 'accepted'
            order by updated_at desc, created_at desc
            limit 1
            "#,
        )
        .bind(tenant_id.0)
        .bind(public_url)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_static_page_draft_row).transpose()
    }

    pub async fn list_accepted_baselines(
        &self,
        tenant_id: TenantId,
        limit: i64,
    ) -> Result<Vec<StaticPageDraft>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, assistant_run_id, title, status, selected_scope,
                   visibility_snapshot, source_refs, draft_payload, created_at, updated_at
            from static_page_drafts
            where tenant_id = $1
              and coalesce(
                    nullif(source_refs #>> '{artifact_stability,baseline_status}', ''),
                    nullif(source_refs #>> '{artifact_stability,baselineStatus}', ''),
                    nullif(draft_payload #>> '{artifact_stability,baseline_status}', ''),
                    nullif(draft_payload #>> '{artifact_stability,baselineStatus}', ''),
                    nullif(draft_payload ->> 'baseline_status', ''),
                    nullif(draft_payload ->> 'baselineStatus', ''),
                    nullif(draft_payload #>> '{artifactStability,baselineStatus}', ''),
                    nullif(draft_payload #>> '{artifactStability,baseline_status}', ''),
                    nullif(draft_payload #>> '{finalPage,baselineStatus}', ''),
                    nullif(draft_payload #>> '{finalPage,baseline_status}', ''),
                    nullif(draft_payload #>> '{final_page,baseline_status}', ''),
                    nullif(draft_payload #>> '{final_page,baselineStatus}', '')
              ) = 'accepted'
            order by updated_at desc, created_at desc
            limit $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_static_page_draft_row).collect()
    }

    pub async fn update(
        &self,
        tenant_id: TenantId,
        draft: &StaticPageDraft,
    ) -> Result<StaticPageDraft> {
        let row = sqlx::query(
            r#"
            update static_page_drafts
            set title = $3,
                status = $4,
                selected_scope = $5,
                visibility_snapshot = $6,
                source_refs = $7,
                draft_payload = $8,
                updated_at = now()
            where tenant_id = $1 and id = $2
            returning id, tenant_id, owner_user_id, assistant_run_id, title, status, selected_scope,
                      visibility_snapshot, source_refs, draft_payload, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(draft.id.0)
        .bind(&draft.title)
        .bind(draft.status.as_str())
        .bind(&draft.selected_scope)
        .bind(&draft.visibility_snapshot)
        .bind(&draft.source_refs)
        .bind(&draft.draft_payload)
        .fetch_one(&self.pool)
        .await?;

        map_static_page_draft_row(&row)
    }
}

#[derive(Clone)]
pub struct PgStaticPageImageJobRepository {
    pool: PgPool,
}

impl PgStaticPageImageJobRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_job: &NewStaticPageImageJob,
    ) -> Result<StaticPageImageJob> {
        let row = sqlx::query(
            r#"
            insert into static_page_image_jobs (
                tenant_id,
                draft_id,
                assistant_run_id,
                status,
                queue_position,
                image_prompt_payload,
                preview_asset_key,
                failure_reason,
                confirmed_at,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            returning id, tenant_id, draft_id, assistant_run_id, status, queue_position,
                      image_prompt_payload, preview_asset_key, failure_reason, confirmed_at,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_job.draft_id.0)
        .bind(new_job.assistant_run_id.0)
        .bind(new_job.status.as_str())
        .bind(new_job.queue_position)
        .bind(&new_job.image_prompt_payload)
        .bind(&new_job.preview_asset_key)
        .bind(&new_job.failure_reason)
        .bind(new_job.confirmed_at)
        .bind(new_job.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_static_page_image_job_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        job_id: StaticPageImageJobId,
    ) -> Result<Option<StaticPageImageJob>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, draft_id, assistant_run_id, status, queue_position,
                   image_prompt_payload, preview_asset_key, failure_reason, confirmed_at,
                   created_at, updated_at
            from static_page_image_jobs
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(job_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_static_page_image_job_row).transpose()
    }

    pub async fn list_by_draft(
        &self,
        tenant_id: TenantId,
        draft_id: StaticPageDraftId,
    ) -> Result<Vec<StaticPageImageJob>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, draft_id, assistant_run_id, status, queue_position,
                   image_prompt_payload, preview_asset_key, failure_reason, confirmed_at,
                   created_at, updated_at
            from static_page_image_jobs
            where tenant_id = $1 and draft_id = $2
            order by updated_at desc, created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(draft_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_static_page_image_job_row).collect()
    }

    pub async fn update(
        &self,
        tenant_id: TenantId,
        job: &StaticPageImageJob,
    ) -> Result<StaticPageImageJob> {
        let row = sqlx::query(
            r#"
            update static_page_image_jobs
            set status = $3,
                queue_position = $4,
                image_prompt_payload = $5,
                preview_asset_key = $6,
                failure_reason = $7,
                confirmed_at = $8,
                updated_at = now()
            where tenant_id = $1 and id = $2
            returning id, tenant_id, draft_id, assistant_run_id, status, queue_position,
                      image_prompt_payload, preview_asset_key, failure_reason, confirmed_at,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(job.id.0)
        .bind(job.status.as_str())
        .bind(job.queue_position)
        .bind(&job.image_prompt_payload)
        .bind(&job.preview_asset_key)
        .bind(&job.failure_reason)
        .bind(job.confirmed_at)
        .fetch_one(&self.pool)
        .await?;

        map_static_page_image_job_row(&row)
    }
}

#[derive(Clone)]
pub struct PgStaticPageRenderOutputRepository {
    pool: PgPool,
}

impl PgStaticPageRenderOutputRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_output: &NewStaticPageRenderOutput,
    ) -> Result<StaticPageRenderOutput> {
        let row = sqlx::query(
            r#"
            insert into static_page_render_outputs (
                tenant_id,
                owner_user_id,
                draft_id,
                assistant_run_id,
                image_job_id,
                status,
                html,
                asset_manifest,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            returning id, tenant_id, owner_user_id, draft_id, assistant_run_id, image_job_id, status,
                      html, asset_manifest, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_output.owner_user_id.map(|id| id.0))
        .bind(new_output.draft_id.0)
        .bind(new_output.assistant_run_id.0)
        .bind(new_output.image_job_id.map(|id| id.0))
        .bind(new_output.status.as_str())
        .bind(&new_output.html)
        .bind(&new_output.asset_manifest)
        .bind(new_output.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_static_page_render_output_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        output_id: StaticPageRenderOutputId,
    ) -> Result<Option<StaticPageRenderOutput>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, draft_id, assistant_run_id, image_job_id, status,
                   html, asset_manifest, created_at
            from static_page_render_outputs
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(output_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref()
            .map(map_static_page_render_output_row)
            .transpose()
    }

    pub async fn update(
        &self,
        tenant_id: TenantId,
        output: &StaticPageRenderOutput,
    ) -> Result<StaticPageRenderOutput> {
        let row = sqlx::query(
            r#"
            update static_page_render_outputs
            set status = $3,
                html = $4,
                asset_manifest = $5
            where tenant_id = $1 and id = $2
            returning id, tenant_id, owner_user_id, draft_id, assistant_run_id, image_job_id, status,
                      html, asset_manifest, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(output.id.0)
        .bind(output.status.as_str())
        .bind(&output.html)
        .bind(&output.asset_manifest)
        .fetch_one(&self.pool)
        .await?;

        map_static_page_render_output_row(&row)
    }

    pub async fn list_by_draft(
        &self,
        tenant_id: TenantId,
        draft_id: StaticPageDraftId,
    ) -> Result<Vec<StaticPageRenderOutput>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, owner_user_id, draft_id, assistant_run_id, image_job_id, status,
                   html, asset_manifest, created_at
            from static_page_render_outputs
            where tenant_id = $1 and draft_id = $2
            order by created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(draft_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_static_page_render_output_row).collect()
    }
}

#[derive(Clone)]
pub struct PgChatSessionRepository {
    pool: PgPool,
}

impl PgChatSessionRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_session: &NewChatSession,
    ) -> Result<ChatSession> {
        let row = sqlx::query(
            r#"
            insert into chat_sessions (
                id,
                tenant_id,
                dataset_id,
                user_id,
                execution_id,
                title,
                latest_memory_directory_id,
                latest_dataset_output_id,
                session_manifest,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            returning id, tenant_id, dataset_id, user_id, execution_id, title, latest_memory_directory_id,
                      latest_dataset_output_id, session_manifest, created_at, updated_at
            "#,
        )
        .bind(new_session.id.0)
        .bind(tenant_id.0)
        .bind(new_session.dataset_id.0)
        .bind(new_session.user_id.map(|id| id.0))
        .bind(new_session.execution_id.0)
        .bind(&new_session.title)
        .bind(new_session.latest_memory_directory_id.map(|value| value.0))
        .bind(new_session.latest_dataset_output_id.map(|value| value.0))
        .bind(&new_session.session_manifest)
        .bind(new_session.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_chat_session_row(&row)
    }

    pub async fn list_by_dataset(
        &self,
        tenant_id: TenantId,
        dataset_id: DatasetId,
    ) -> Result<Vec<ChatSession>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, user_id, execution_id, title, latest_memory_directory_id,
                   latest_dataset_output_id, session_manifest, created_at, updated_at
            from chat_sessions
            where tenant_id = $1 and dataset_id = $2
            order by created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_chat_session_row).collect()
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        session_id: ChatSessionId,
    ) -> Result<Option<ChatSession>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, user_id, execution_id, title, latest_memory_directory_id,
                   latest_dataset_output_id, session_manifest, created_at, updated_at
            from chat_sessions
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(session_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_chat_session_row).transpose()
    }

    pub async fn get_by_execution_id(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
    ) -> Result<Option<ChatSession>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, user_id, execution_id, title, latest_memory_directory_id,
                   latest_dataset_output_id, session_manifest, created_at, updated_at
            from chat_sessions
            where tenant_id = $1 and execution_id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(execution_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_chat_session_row).transpose()
    }

    pub async fn update_context(
        &self,
        tenant_id: TenantId,
        session_id: ChatSessionId,
        latest_memory_directory_id: Option<MemoryDirectoryId>,
        latest_dataset_output_id: Option<DatasetOutputId>,
        session_manifest: &Value,
        updated_at: DateTime<Utc>,
    ) -> Result<ChatSession> {
        let row = sqlx::query(
            r#"
            update chat_sessions
            set latest_memory_directory_id = $3,
                latest_dataset_output_id = $4,
                session_manifest = $5,
                updated_at = $6
            where tenant_id = $1 and id = $2
            returning id, tenant_id, dataset_id, user_id, execution_id, title, latest_memory_directory_id,
                      latest_dataset_output_id, session_manifest, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(session_id.0)
        .bind(latest_memory_directory_id.map(|value| value.0))
        .bind(latest_dataset_output_id.map(|value| value.0))
        .bind(session_manifest)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_chat_session_row(&row)
    }

    pub async fn update_title(
        &self,
        tenant_id: TenantId,
        session_id: ChatSessionId,
        title: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<ChatSession> {
        let row = sqlx::query(
            r#"
            update chat_sessions
            set title = $3,
                updated_at = $4
            where tenant_id = $1 and id = $2
            returning id, tenant_id, dataset_id, user_id, execution_id, title, latest_memory_directory_id,
                      latest_dataset_output_id, session_manifest, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(session_id.0)
        .bind(title)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_chat_session_row(&row)
    }
}

#[derive(Clone)]
pub struct PgChatMessageRepository {
    pool: PgPool,
}

impl PgChatMessageRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_message: &NewChatMessage,
    ) -> Result<ChatMessage> {
        let row = sqlx::query(
            r#"
            insert into chat_messages (
                id,
                tenant_id,
                session_id,
                role,
                turn_index,
                content,
                message_manifest,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            returning id, tenant_id, session_id, role, turn_index, content, message_manifest, created_at
            "#,
        )
        .bind(ChatMessageId::new().0)
        .bind(tenant_id.0)
        .bind(new_message.session_id.0)
        .bind(new_message.role.as_str())
        .bind(new_message.turn_index)
        .bind(&new_message.content)
        .bind(&new_message.message_manifest)
        .bind(new_message.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_chat_message_row(&row)
    }

    pub async fn list_by_session(
        &self,
        tenant_id: TenantId,
        session_id: ChatSessionId,
    ) -> Result<Vec<ChatMessage>> {
        let rows = sqlx::query(
            r#"
            select m.id, m.tenant_id, m.session_id, m.role, m.turn_index, m.content, m.message_manifest, m.created_at
            from chat_messages m
            join chat_sessions s on s.id = m.session_id
            where s.tenant_id = $1 and m.session_id = $2
            order by m.turn_index asc, m.created_at asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(session_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_chat_message_row).collect()
    }

    pub async fn update_manifest(
        &self,
        tenant_id: TenantId,
        message_id: ChatMessageId,
        message_manifest: &Value,
    ) -> Result<ChatMessage> {
        let row = sqlx::query(
            r#"
            update chat_messages
            set message_manifest = $3
            where tenant_id = $1 and id = $2
            returning id, tenant_id, session_id, role, turn_index, content, message_manifest, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(message_id.0)
        .bind(message_manifest)
        .fetch_one(&self.pool)
        .await?;

        map_chat_message_row(&row)
    }
}

#[derive(Clone)]
pub struct PgLlmInvocationRepository {
    pool: PgPool,
}

impl PgLlmInvocationRepository {
    pub async fn replace_for_dataset_output(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        dataset_output_id: DatasetOutputId,
        manifest: &Value,
        created_at: DateTime<Utc>,
    ) -> Result<Vec<LlmInvocation>> {
        let invocations = parse_manifest_llm_invocations(manifest)?;
        self.replace_for_dataset_output_records(
            tenant_id,
            execution_id,
            dataset_output_id,
            &invocations,
            created_at,
        )
        .await
    }

    pub async fn replace_for_dataset_output_records(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        dataset_output_id: DatasetOutputId,
        invocations: &[LlmInvocationRecordInput],
        created_at: DateTime<Utc>,
    ) -> Result<Vec<LlmInvocation>> {
        self.replace(
            tenant_id,
            execution_id,
            LlmInvocationReplaceTarget::DatasetOutput(dataset_output_id),
            invocations,
            created_at,
        )
        .await
    }

    pub async fn replace_for_chat_message(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        chat_message_id: ChatMessageId,
        manifest: &Value,
        created_at: DateTime<Utc>,
    ) -> Result<Vec<LlmInvocation>> {
        let invocations = parse_manifest_llm_invocations(manifest)?;
        self.replace_for_chat_message_records(
            tenant_id,
            execution_id,
            chat_message_id,
            &invocations,
            created_at,
        )
        .await
    }

    pub async fn replace_for_chat_message_records(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        chat_message_id: ChatMessageId,
        invocations: &[LlmInvocationRecordInput],
        created_at: DateTime<Utc>,
    ) -> Result<Vec<LlmInvocation>> {
        self.replace(
            tenant_id,
            execution_id,
            LlmInvocationReplaceTarget::ChatMessage(chat_message_id),
            invocations,
            created_at,
        )
        .await
    }

    pub async fn replace_for_execution_records(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        invocations: &[LlmInvocationRecordInput],
        created_at: DateTime<Utc>,
    ) -> Result<Vec<LlmInvocation>> {
        self.replace(
            tenant_id,
            execution_id,
            LlmInvocationReplaceTarget::WorkflowExecution,
            invocations,
            created_at,
        )
        .await
    }

    pub async fn list_by_execution(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
    ) -> Result<Vec<LlmInvocation>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, source_kind, dataset_output_id, chat_message_id,
                   sequence_no, mode, provider, model, request_id, finish_reason, latency_ms,
                   usage, system_prompt_key, system_prompt_version, tool_trace_count, created_at
            from llm_invocations
            where tenant_id = $1 and execution_id = $2
            order by source_kind asc, sequence_no asc, created_at asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(execution_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_llm_invocation_row).collect()
    }

    pub async fn list_by_dataset_output(
        &self,
        tenant_id: TenantId,
        dataset_output_id: DatasetOutputId,
    ) -> Result<Vec<LlmInvocation>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, source_kind, dataset_output_id, chat_message_id,
                   sequence_no, mode, provider, model, request_id, finish_reason, latency_ms,
                   usage, system_prompt_key, system_prompt_version, tool_trace_count, created_at
            from llm_invocations
            where tenant_id = $1 and dataset_output_id = $2
            order by sequence_no asc, created_at asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_output_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_llm_invocation_row).collect()
    }

    pub async fn list_by_chat_message(
        &self,
        tenant_id: TenantId,
        chat_message_id: ChatMessageId,
    ) -> Result<Vec<LlmInvocation>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, source_kind, dataset_output_id, chat_message_id,
                   sequence_no, mode, provider, model, request_id, finish_reason, latency_ms,
                   usage, system_prompt_key, system_prompt_version, tool_trace_count, created_at
            from llm_invocations
            where tenant_id = $1 and chat_message_id = $2
            order by sequence_no asc, created_at asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(chat_message_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_llm_invocation_row).collect()
    }

    async fn replace(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        target: LlmInvocationReplaceTarget,
        invocations: &[LlmInvocationRecordInput],
        created_at: DateTime<Utc>,
    ) -> Result<Vec<LlmInvocation>> {
        let mut tx = self.pool.begin().await?;

        match target {
            LlmInvocationReplaceTarget::DatasetOutput(dataset_output_id) => {
                sqlx::query(
                    r#"
                    delete from llm_invocations
                    where tenant_id = $1 and dataset_output_id = $2
                    "#,
                )
                .bind(tenant_id.0)
                .bind(dataset_output_id.0)
                .execute(&mut *tx)
                .await?;
            }
            LlmInvocationReplaceTarget::ChatMessage(chat_message_id) => {
                sqlx::query(
                    r#"
                    delete from llm_invocations
                    where tenant_id = $1 and chat_message_id = $2
                    "#,
                )
                .bind(tenant_id.0)
                .bind(chat_message_id.0)
                .execute(&mut *tx)
                .await?;
            }
            LlmInvocationReplaceTarget::WorkflowExecution => {
                sqlx::query(
                    r#"
                    delete from llm_invocations
                    where tenant_id = $1 and execution_id = $2 and source_kind = 'workflow_execution'
                    "#,
                )
                .bind(tenant_id.0)
                .bind(execution_id.0)
                .execute(&mut *tx)
                .await?;
            }
        }

        let mut persisted = Vec::with_capacity(invocations.len());
        for (sequence_no, invocation) in invocations.iter().enumerate() {
            let source_kind = match target {
                LlmInvocationReplaceTarget::DatasetOutput(_) => {
                    LlmInvocationSourceKind::DatasetOutput
                }
                LlmInvocationReplaceTarget::ChatMessage(_) => LlmInvocationSourceKind::ChatMessage,
                LlmInvocationReplaceTarget::WorkflowExecution => {
                    LlmInvocationSourceKind::WorkflowExecution
                }
            };
            let usage = invocation.usage.as_ref().map(|usage| {
                serde_json::json!({
                    "input_tokens": usage.input_tokens,
                    "output_tokens": usage.output_tokens,
                    "total_tokens": usage.total_tokens,
                })
            });
            let row = sqlx::query(
                r#"
                insert into llm_invocations (
                    id,
                    tenant_id,
                    execution_id,
                    source_kind,
                    dataset_output_id,
                    chat_message_id,
                    sequence_no,
                    mode,
                    provider,
                    model,
                    request_id,
                    finish_reason,
                    latency_ms,
                    usage,
                    system_prompt_key,
                    system_prompt_version,
                    tool_trace_count,
                    created_at
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                returning id, tenant_id, execution_id, source_kind, dataset_output_id,
                          chat_message_id, sequence_no, mode, provider, model, request_id,
                          finish_reason, latency_ms, usage, system_prompt_key,
                          system_prompt_version, tool_trace_count, created_at
                "#,
            )
            .bind(LlmInvocationId::new().0)
            .bind(tenant_id.0)
            .bind(execution_id.0)
            .bind(source_kind.as_str())
            .bind(match target {
                LlmInvocationReplaceTarget::DatasetOutput(dataset_output_id) => {
                    Some(dataset_output_id.0)
                }
                LlmInvocationReplaceTarget::ChatMessage(_)
                | LlmInvocationReplaceTarget::WorkflowExecution => None,
            })
            .bind(match target {
                LlmInvocationReplaceTarget::DatasetOutput(_) => None,
                LlmInvocationReplaceTarget::ChatMessage(chat_message_id) => {
                    Some(chat_message_id.0)
                }
                LlmInvocationReplaceTarget::WorkflowExecution => None,
            })
            .bind(sequence_no as i32)
            .bind(invocation.mode.as_str())
            .bind(&invocation.provider)
            .bind(&invocation.model)
            .bind(&invocation.request_id)
            .bind(invocation.finish_reason.as_ref().map(|reason| reason.as_str()))
            .bind(invocation.latency_ms.map(|value| value as i64))
            .bind(&usage)
            .bind(&invocation.system_prompt_key)
            .bind(&invocation.system_prompt_version)
            .bind(invocation.tool_trace_count.map(|value| value as i32))
            .bind(created_at)
            .fetch_one(&mut *tx)
            .await?;

            persisted.push(map_llm_invocation_row(&row)?);
        }

        tx.commit().await?;
        Ok(persisted)
    }
}

#[derive(Clone)]
pub struct PgToolExecutionRepository {
    pool: PgPool,
}

impl PgToolExecutionRepository {
    pub async fn replace_for_dataset_output(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        dataset_output_id: DatasetOutputId,
        manifest: &Value,
        created_at: DateTime<Utc>,
    ) -> Result<Vec<ToolExecution>> {
        let tool_calls = parse_manifest_tool_calls(manifest)?;
        self.replace_for_dataset_output_records(
            tenant_id,
            execution_id,
            dataset_output_id,
            &tool_calls,
            created_at,
        )
        .await
    }

    pub async fn replace_for_dataset_output_records(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        dataset_output_id: DatasetOutputId,
        tool_calls: &[ToolExecutionRecordInput],
        created_at: DateTime<Utc>,
    ) -> Result<Vec<ToolExecution>> {
        self.replace(
            tenant_id,
            execution_id,
            ToolExecutionReplaceTarget::DatasetOutput(dataset_output_id),
            tool_calls,
            created_at,
        )
        .await
    }

    pub async fn replace_for_chat_message(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        chat_message_id: ChatMessageId,
        manifest: &Value,
        created_at: DateTime<Utc>,
    ) -> Result<Vec<ToolExecution>> {
        let tool_calls = parse_manifest_tool_calls(manifest)?;
        self.replace_for_chat_message_records(
            tenant_id,
            execution_id,
            chat_message_id,
            &tool_calls,
            created_at,
        )
        .await
    }

    pub async fn replace_for_chat_message_records(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        chat_message_id: ChatMessageId,
        tool_calls: &[ToolExecutionRecordInput],
        created_at: DateTime<Utc>,
    ) -> Result<Vec<ToolExecution>> {
        self.replace(
            tenant_id,
            execution_id,
            ToolExecutionReplaceTarget::ChatMessage(chat_message_id),
            tool_calls,
            created_at,
        )
        .await
    }

    pub async fn replace_for_execution_records(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        tool_calls: &[ToolExecutionRecordInput],
        created_at: DateTime<Utc>,
    ) -> Result<Vec<ToolExecution>> {
        self.replace(
            tenant_id,
            execution_id,
            ToolExecutionReplaceTarget::WorkflowExecution,
            tool_calls,
            created_at,
        )
        .await
    }

    pub async fn list_by_execution(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
    ) -> Result<Vec<ToolExecution>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, source_kind, dataset_output_id, chat_message_id,
                   sequence_no, call_id, tool_name, tool_snapshot, status, arguments, result,
                   created_at
            from tool_executions
            where tenant_id = $1 and execution_id = $2
            order by source_kind asc, sequence_no asc, created_at asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(execution_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_tool_execution_row).collect()
    }

    pub async fn list_by_dataset_output(
        &self,
        tenant_id: TenantId,
        dataset_output_id: DatasetOutputId,
    ) -> Result<Vec<ToolExecution>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, source_kind, dataset_output_id, chat_message_id,
                   sequence_no, call_id, tool_name, tool_snapshot, status, arguments, result,
                   created_at
            from tool_executions
            where tenant_id = $1 and dataset_output_id = $2
            order by sequence_no asc, created_at asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(dataset_output_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_tool_execution_row).collect()
    }

    pub async fn list_by_chat_message(
        &self,
        tenant_id: TenantId,
        chat_message_id: ChatMessageId,
    ) -> Result<Vec<ToolExecution>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, source_kind, dataset_output_id, chat_message_id,
                   sequence_no, call_id, tool_name, tool_snapshot, status, arguments, result,
                   created_at
            from tool_executions
            where tenant_id = $1 and chat_message_id = $2
            order by sequence_no asc, created_at asc
            "#,
        )
        .bind(tenant_id.0)
        .bind(chat_message_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_tool_execution_row).collect()
    }

    async fn replace(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        target: ToolExecutionReplaceTarget,
        tool_calls: &[ToolExecutionRecordInput],
        created_at: DateTime<Utc>,
    ) -> Result<Vec<ToolExecution>> {
        let mut tx = self.pool.begin().await?;

        match target {
            ToolExecutionReplaceTarget::DatasetOutput(dataset_output_id) => {
                sqlx::query(
                    r#"
                    delete from tool_executions
                    where tenant_id = $1 and dataset_output_id = $2
                    "#,
                )
                .bind(tenant_id.0)
                .bind(dataset_output_id.0)
                .execute(&mut *tx)
                .await?;
            }
            ToolExecutionReplaceTarget::ChatMessage(chat_message_id) => {
                sqlx::query(
                    r#"
                    delete from tool_executions
                    where tenant_id = $1 and chat_message_id = $2
                    "#,
                )
                .bind(tenant_id.0)
                .bind(chat_message_id.0)
                .execute(&mut *tx)
                .await?;
            }
            ToolExecutionReplaceTarget::WorkflowExecution => {
                sqlx::query(
                    r#"
                    delete from tool_executions
                    where tenant_id = $1 and execution_id = $2 and source_kind = 'workflow_execution'
                    "#,
                )
                .bind(tenant_id.0)
                .bind(execution_id.0)
                .execute(&mut *tx)
                .await?;
            }
        }

        let mut persisted = Vec::with_capacity(tool_calls.len());
        for (sequence_no, tool_call) in tool_calls.iter().enumerate() {
            let source_kind = match target {
                ToolExecutionReplaceTarget::DatasetOutput(_) => {
                    ToolExecutionSourceKind::DatasetOutput
                }
                ToolExecutionReplaceTarget::ChatMessage(_) => ToolExecutionSourceKind::ChatMessage,
                ToolExecutionReplaceTarget::WorkflowExecution => {
                    ToolExecutionSourceKind::WorkflowExecution
                }
            };
            let row = sqlx::query(
                r#"
                insert into tool_executions (
                    id,
                    tenant_id,
                    execution_id,
                    source_kind,
                    dataset_output_id,
                    chat_message_id,
                    sequence_no,
                    call_id,
                    tool_name,
                    tool_snapshot,
                    status,
                    arguments,
                    result,
                    created_at
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                returning id, tenant_id, execution_id, source_kind, dataset_output_id,
                          chat_message_id, sequence_no, call_id, tool_name, tool_snapshot,
                          status, arguments, result, created_at
                "#,
            )
            .bind(ToolExecutionId::new().0)
            .bind(tenant_id.0)
            .bind(execution_id.0)
            .bind(source_kind.as_str())
            .bind(match target {
                ToolExecutionReplaceTarget::DatasetOutput(dataset_output_id) => {
                    Some(dataset_output_id.0)
                }
                ToolExecutionReplaceTarget::ChatMessage(_)
                | ToolExecutionReplaceTarget::WorkflowExecution => None,
            })
            .bind(match target {
                ToolExecutionReplaceTarget::DatasetOutput(_) => None,
                ToolExecutionReplaceTarget::ChatMessage(chat_message_id) => Some(chat_message_id.0),
                ToolExecutionReplaceTarget::WorkflowExecution => None,
            })
            .bind(sequence_no as i32)
            .bind(&tool_call.call_id)
            .bind(&tool_call.tool_name)
            .bind(&tool_call.tool_snapshot)
            .bind(tool_call.status.as_str())
            .bind(&tool_call.arguments)
            .bind(&tool_call.result)
            .bind(created_at)
            .fetch_one(&mut *tx)
            .await?;

            persisted.push(map_tool_execution_row(&row)?);
        }

        tx.commit().await?;
        Ok(persisted)
    }
}

#[derive(Clone)]
pub struct PgWorkflowExecutionRepository {
    pool: PgPool,
}

impl PgWorkflowExecutionRepository {
    pub async fn create(&self, execution: &WorkflowExecution) -> Result<()> {
        sqlx::query(
            r#"
            insert into workflow_executions (
                id,
                tenant_id,
                dataset_id,
                report_plan_id,
                kind,
                version,
                stage,
                status,
                attempt,
                context,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(execution.id.0)
        .bind(execution.tenant_id.0)
        .bind(execution.dataset_id.map(|value| value.0))
        .bind(execution.report_plan_id.map(|value| value.0))
        .bind(execution.kind.as_str())
        .bind(&execution.version)
        .bind(&execution.stage)
        .bind(execution.status.as_str())
        .bind(execution.attempt as i32)
        .bind(&execution.context)
        .bind(execution.created_at)
        .bind(execution.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_with_initial_event(
        &self,
        execution: &WorkflowExecution,
        event: &WorkflowEventRecord,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        insert_workflow_execution(&mut *tx, execution).await?;
        insert_workflow_event(&mut *tx, event).await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
    ) -> Result<Option<WorkflowExecution>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, report_plan_id, kind, version, stage, status, attempt, context, created_at, updated_at
            from workflow_executions
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(execution_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_workflow_execution_row).transpose()
    }

    pub async fn list_by_tenant(&self, tenant_id: TenantId) -> Result<Vec<WorkflowExecution>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, report_plan_id, kind, version, stage, status, attempt, context, created_at, updated_at
            from workflow_executions
            where tenant_id = $1
            order by created_at desc
            "#,
        )
        .bind(tenant_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_workflow_execution_row).collect()
    }

    pub async fn update_detached_task_state(
        &self,
        tenant_id: TenantId,
        execution_id: WorkflowExecutionId,
        status: WorkflowStatus,
        stage: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            update workflow_executions
            set status = $3, stage = $4, updated_at = $5
            where tenant_id = $1 and id = $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(execution_id.0)
        .bind(status.as_str())
        .bind(stage)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_latest_by_report_plan_and_kind(
        &self,
        tenant_id: TenantId,
        report_plan_id: ReportPlanId,
        kind: WorkflowKind,
    ) -> Result<Option<WorkflowExecution>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, report_plan_id, kind, version, stage, status, attempt, context, created_at, updated_at
            from workflow_executions
            where tenant_id = $1 and report_plan_id = $2 and kind = $3
            order by created_at desc
            limit 1
            "#,
        )
        .bind(tenant_id.0)
        .bind(report_plan_id.0)
        .bind(kind.as_str())
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_workflow_execution_row).transpose()
    }

    pub async fn advance(
        &self,
        execution: &WorkflowExecution,
        event_name: &str,
        event_payload: &Value,
        occurred_at: DateTime<Utc>,
    ) -> Result<WorkflowEventRecord> {
        let (event, _) = self
            .advance_with_tasks(execution, event_name, event_payload, occurred_at, &[])
            .await?;

        Ok(event)
    }

    pub async fn advance_with_tasks(
        &self,
        execution: &WorkflowExecution,
        event_name: &str,
        event_payload: &Value,
        occurred_at: DateTime<Utc>,
        tasks: &[NewWorkflowTask],
    ) -> Result<(WorkflowEventRecord, Vec<WorkflowTask>)> {
        let mut tx = self.pool.begin().await?;
        let next_sequence = sqlx::query_scalar::<_, i64>(
            r#"
            select coalesce(max(sequence_no), 0) + 1
            from workflow_events
            where execution_id = $1
            "#,
        )
        .bind(execution.id.0)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            update workflow_executions
            set version = $2,
                stage = $3,
                status = $4,
                attempt = $5,
                context = $6,
                updated_at = $7
            where id = $1 and tenant_id = $8
            "#,
        )
        .bind(execution.id.0)
        .bind(&execution.version)
        .bind(&execution.stage)
        .bind(execution.status.as_str())
        .bind(execution.attempt as i32)
        .bind(&execution.context)
        .bind(execution.updated_at)
        .bind(execution.tenant_id.0)
        .execute(&mut *tx)
        .await?;

        let event = WorkflowEventRecord {
            id: WorkflowEventId::new(),
            execution_id: execution.id,
            sequence_no: next_sequence,
            event_name: event_name.to_string(),
            payload: event_payload.clone(),
            created_at: occurred_at,
        };
        insert_workflow_event(&mut *tx, &event).await?;

        let mut persisted_tasks = Vec::with_capacity(tasks.len());
        for task in tasks {
            let task = insert_workflow_task(&mut *tx, execution, task, occurred_at).await?;
            persisted_tasks.push(task);
        }

        tx.commit().await?;
        Ok((event, persisted_tasks))
    }
}

#[derive(Clone)]
pub struct PgWorkflowEventRepository {
    pool: PgPool,
}

impl PgWorkflowEventRepository {
    pub async fn create(
        &self,
        execution_id: WorkflowExecutionId,
        event_name: &str,
        payload: &Value,
        created_at: DateTime<Utc>,
    ) -> Result<WorkflowEventRecord> {
        let sequence_no: i64 = sqlx::query_scalar(
            r#"
            select coalesce(max(sequence_no), 0) + 1
            from workflow_events
            where execution_id = $1
            "#,
        )
        .bind(execution_id.0)
        .fetch_one(&self.pool)
        .await?;

        let event = WorkflowEventRecord {
            id: WorkflowEventId::new(),
            execution_id,
            sequence_no,
            event_name: event_name.to_string(),
            payload: payload.clone(),
            created_at,
        };
        insert_workflow_event(&self.pool, &event).await?;
        Ok(event)
    }

    pub async fn list_by_execution(
        &self,
        execution_id: WorkflowExecutionId,
    ) -> Result<Vec<WorkflowEventRecord>> {
        let rows = sqlx::query(
            r#"
            select id, execution_id, sequence_no, event_name, payload, created_at
            from workflow_events
            where execution_id = $1
            order by sequence_no asc
            "#,
        )
        .bind(execution_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_workflow_event_row).collect()
    }
}

#[derive(Clone)]
pub struct PgWorkflowTaskRepository {
    pool: PgPool,
}

impl PgWorkflowTaskRepository {
    pub async fn create(
        &self,
        execution: &WorkflowExecution,
        task: &NewWorkflowTask,
        created_at: DateTime<Utc>,
    ) -> Result<WorkflowTask> {
        insert_workflow_task(&self.pool, execution, task, created_at).await
    }

    pub async fn create_asset_parse_if_absent(
        &self,
        execution: &WorkflowExecution,
        initial_event: &WorkflowEventRecord,
        task: &NewWorkflowTask,
        dedupe_key: &str,
        created_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowTask>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(dedupe_key)
            .execute(&mut *tx)
            .await?;

        let existing = sqlx::query(
            r#"
            select id, tenant_id, execution_id, queue, task_key, payload, status, attempt,
                   max_attempts, available_at, claimed_at, finished_at, error, created_at, updated_at
            from workflow_tasks
            where tenant_id = $1
              and queue = $2
              and task_key = $3
              and status in ('queued', 'claimed')
              and payload ->> 'dedupe_key' = $4
            order by created_at asc
            limit 1
            "#,
        )
        .bind(execution.tenant_id.0)
        .bind(&task.queue)
        .bind(&task.task_key)
        .bind(dedupe_key)
        .fetch_optional(&mut *tx)
        .await?;
        if existing.is_some() {
            tx.commit().await?;
            return Ok(None);
        }

        insert_workflow_execution(&mut *tx, execution).await?;
        insert_workflow_event(&mut *tx, initial_event).await?;
        let persisted = insert_workflow_task(&mut *tx, execution, task, created_at).await?;
        tx.commit().await?;
        Ok(Some(persisted))
    }

    pub async fn list_by_execution(
        &self,
        execution_id: WorkflowExecutionId,
    ) -> Result<Vec<WorkflowTask>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                   available_at, claimed_at, finished_at, error, created_at, updated_at
            from workflow_tasks
            where execution_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(execution_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_workflow_task_row).collect()
    }

    pub async fn claim_next_available(
        &self,
        queue: &str,
        task_key: Option<&str>,
        claimed_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowTask>> {
        let row = sqlx::query(
            r#"
            with next_task as (
                select id
                from workflow_tasks
                where queue = $1
                  and status = 'queued'
                  and available_at <= $2
                  and ($3::text is null or task_key = $3)
                order by available_at asc, created_at asc
                for update skip locked
                limit 1
            )
            update workflow_tasks
            set status = 'claimed',
                attempt = attempt + 1,
                claimed_at = $2,
                updated_at = $2,
                error = null
            where id in (select id from next_task)
            returning id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                      available_at, claimed_at, finished_at, error, created_at, updated_at
            "#,
        )
        .bind(queue)
        .bind(claimed_at)
        .bind(task_key)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_workflow_task_row).transpose()
    }

    pub async fn list_stale_claimed(
        &self,
        queue: &str,
        task_key: Option<&str>,
        claimed_before: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<WorkflowTask>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                   available_at, claimed_at, finished_at, error, created_at, updated_at
            from workflow_tasks
            where queue = $1
              and status = 'claimed'
              and claimed_at is not null
              and claimed_at < $2
              and ($3::text is null or task_key = $3)
            order by claimed_at asc, created_at asc, id asc
            limit $4
            "#,
        )
        .bind(queue)
        .bind(claimed_before)
        .bind(task_key)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_workflow_task_row).collect()
    }

    pub async fn mark_succeeded(
        &self,
        task_id: WorkflowTaskId,
        finished_at: DateTime<Utc>,
    ) -> Result<WorkflowTask> {
        let row = sqlx::query(
            r#"
            update workflow_tasks
            set status = 'succeeded',
                finished_at = $2,
                updated_at = $2,
                error = null
            where id = $1
            returning id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                      available_at, claimed_at, finished_at, error, created_at, updated_at
            "#,
        )
        .bind(task_id.0)
        .bind(finished_at)
        .fetch_one(&self.pool)
        .await?;

        map_workflow_task_row(&row)
    }

    pub async fn update_payload(
        &self,
        task_id: WorkflowTaskId,
        payload: &Value,
        updated_at: DateTime<Utc>,
    ) -> Result<WorkflowTask> {
        let row = sqlx::query(
            r#"
            update workflow_tasks
            set payload = $2,
                updated_at = $3
            where id = $1
            returning id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                      available_at, claimed_at, finished_at, error, created_at, updated_at
            "#,
        )
        .bind(task_id.0)
        .bind(payload)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_workflow_task_row(&row)
    }

    pub async fn update_payload_and_available_at(
        &self,
        task_id: WorkflowTaskId,
        payload: &Value,
        available_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<WorkflowTask> {
        let row = sqlx::query(
            r#"
            update workflow_tasks
            set payload = $2,
                available_at = $3,
                updated_at = $4
            where id = $1
            returning id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                      available_at, claimed_at, finished_at, error, created_at, updated_at
            "#,
        )
        .bind(task_id.0)
        .bind(payload)
        .bind(available_at)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_workflow_task_row(&row)
    }

    pub async fn count_active_static_page_heavy_tasks_excluding(
        &self,
        tenant_id: TenantId,
        excluded_task_id: WorkflowTaskId,
    ) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)::bigint
            from workflow_tasks
            where tenant_id = $1
              and id <> $2
              and status in ('queued', 'claimed')
              and (
                (queue = 'static_page' and task_key in ('generate_static_page_image', 'render_static_page'))
                or queue = 'codex_host'
              )
            "#,
        )
        .bind(tenant_id.0)
        .bind(excluded_task_id.0)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    pub async fn mark_failed(
        &self,
        task_id: WorkflowTaskId,
        error: &str,
        finished_at: DateTime<Utc>,
    ) -> Result<WorkflowTask> {
        let row = sqlx::query(
            r#"
            update workflow_tasks
            set status = 'failed',
                finished_at = $3,
                updated_at = $3,
                error = $2
            where id = $1
            returning id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                      available_at, claimed_at, finished_at, error, created_at, updated_at
            "#,
        )
        .bind(task_id.0)
        .bind(error)
        .bind(finished_at)
        .fetch_one(&self.pool)
        .await?;

        map_workflow_task_row(&row)
    }

    pub async fn requeue_after_transient_error(
        &self,
        task_id: WorkflowTaskId,
        error: &str,
        available_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<WorkflowTask> {
        let row = sqlx::query(
            r#"
            update workflow_tasks
            set status = 'queued',
                available_at = $3,
                claimed_at = null,
                finished_at = null,
                updated_at = $4,
                error = $2
            where id = $1
            returning id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                      available_at, claimed_at, finished_at, error, created_at, updated_at
            "#,
        )
        .bind(task_id.0)
        .bind(error)
        .bind(available_at)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_workflow_task_row(&row)
    }

    pub async fn requeue_after_non_consuming_transient_error(
        &self,
        task_id: WorkflowTaskId,
        error: &str,
        available_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<WorkflowTask> {
        let row = sqlx::query(
            r#"
            update workflow_tasks
            set status = 'queued',
                attempt = greatest(attempt - 1, 0),
                available_at = $3,
                claimed_at = null,
                finished_at = null,
                updated_at = $4,
                error = $2
            where id = $1
            returning id, tenant_id, execution_id, queue, task_key, payload, status, attempt, max_attempts,
                      available_at, claimed_at, finished_at, error, created_at, updated_at
            "#,
        )
        .bind(task_id.0)
        .bind(error)
        .bind(available_at)
        .bind(updated_at)
        .fetch_one(&self.pool)
        .await?;

        map_workflow_task_row(&row)
    }
}

impl PgAuthAuditEventRepository {
    pub async fn create(
        &self,
        tenant_id: TenantId,
        new_event: NewAuthAuditEvent,
    ) -> Result<AuthAuditEvent> {
        let row = sqlx::query(
            r#"
            insert into auth_audit_events (
                tenant_id,
                user_id,
                session_id,
                email_normalized,
                event_name,
                outcome,
                ip_hash,
                device_fingerprint,
                metadata,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            returning id, tenant_id, user_id, session_id, email_normalized, event_name,
                      outcome, ip_hash, device_fingerprint, metadata, created_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_event.user_id.map(|id| id.0))
        .bind(new_event.session_id.map(|id| id.0))
        .bind(new_event.email_normalized)
        .bind(new_event.event_name)
        .bind(new_event.outcome.as_str())
        .bind(new_event.ip_hash)
        .bind(new_event.device_fingerprint)
        .bind(new_event.metadata)
        .bind(new_event.created_at)
        .fetch_one(&self.pool)
        .await?;

        map_auth_audit_event_row(&row)
    }

    pub async fn list_recent_for_user(
        &self,
        tenant_id: TenantId,
        user_id: UserId,
        limit: i64,
    ) -> Result<Vec<AuthAuditEvent>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, user_id, session_id, email_normalized, event_name,
                   outcome, ip_hash, device_fingerprint, metadata, created_at
            from auth_audit_events
            where tenant_id = $1 and user_id = $2
            order by created_at desc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(user_id.0)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_auth_audit_event_row).collect()
    }

    pub async fn list_recent_by_email(
        &self,
        tenant_id: TenantId,
        email_normalized: &str,
        limit: i64,
    ) -> Result<Vec<AuthAuditEvent>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, user_id, session_id, email_normalized, event_name,
                   outcome, ip_hash, device_fingerprint, metadata, created_at
            from auth_audit_events
            where tenant_id = $1 and email_normalized = $2
            order by created_at desc
            limit $3
            "#,
        )
        .bind(tenant_id.0)
        .bind(email_normalized)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_auth_audit_event_row).collect()
    }
}

fn map_auth_audit_event_row(row: &sqlx::postgres::PgRow) -> Result<AuthAuditEvent> {
    let outcome = row.get::<String, _>("outcome");

    Ok(AuthAuditEvent {
        id: AuthAuditEventId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        user_id: row.get::<Option<Uuid>, _>("user_id").map(UserId),
        session_id: row.get::<Option<Uuid>, _>("session_id").map(UserSessionId),
        email_normalized: row.get("email_normalized"),
        event_name: row.get("event_name"),
        outcome: AuthAuditOutcome::from_str(&outcome)
            .ok_or_else(|| anyhow!("unknown auth audit outcome: {outcome}"))?,
        ip_hash: row.get("ip_hash"),
        device_fingerprint: row.get("device_fingerprint"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    })
}

fn map_dataset_row(row: &sqlx::postgres::PgRow) -> Result<Dataset> {
    let lifecycle = row.get::<String, _>("lifecycle");
    let metadata = row.get::<Value, _>("metadata");
    let visibility = dataset_visibility_from_metadata(&metadata);
    let default_secret_binding_ids =
        secret_binding_ids_from_metadata_key(&metadata, "default_secret_binding_ids")?;

    Ok(Dataset {
        id: DatasetId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        key: row.get("key"),
        title: row.get("title"),
        description: row.get("description"),
        lifecycle: DatasetLifecycle::from_str(&lifecycle)
            .ok_or_else(|| anyhow!("unknown dataset lifecycle: {lifecycle}"))?,
        visibility,
        default_secret_binding_ids,
        metadata: json_object_to_btree_map(metadata)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_document_row(row: &sqlx::postgres::PgRow) -> Result<Document> {
    let lifecycle = row.get::<String, _>("lifecycle");
    let metadata = row.get::<Value, _>("metadata");

    Ok(Document {
        id: DocumentId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        title: row.get("title"),
        object_key: row.get("object_key"),
        content_type: row.get("content_type"),
        lifecycle: DocumentLifecycle::from_str(&lifecycle)
            .ok_or_else(|| anyhow!("unknown document lifecycle: {lifecycle}"))?,
        secret_binding_ids: secret_binding_ids_from_metadata(&metadata)?,
        metadata: json_object_to_btree_map(metadata)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_document_enrichment_run_row(row: &sqlx::postgres::PgRow) -> Result<DocumentEnrichmentRun> {
    Ok(DocumentEnrichmentRun {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        document_id: DocumentId(row.get::<Uuid, _>("document_id")),
        enrichment_kind: row.get("enrichment_kind"),
        parse_version: row.get("parse_version"),
        input_fingerprint: row.get("input_fingerprint"),
        status: row.get("status"),
        priority: row.get("priority"),
        attempt_count: row.get("attempt_count"),
        max_attempts: row.get("max_attempts"),
        available_at: row.get("available_at"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        error_message: row.get("error_message"),
        output_summary: row.get("output_summary"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_dataset_document_membership_row(
    row: &sqlx::postgres::PgRow,
) -> Result<DatasetDocumentMembership> {
    Ok(DatasetDocumentMembership {
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        document_id: DocumentId(row.get::<Uuid, _>("document_id")),
        membership_kind: row.get("membership_kind"),
        source: row.get("source"),
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
    })
}

fn map_model_gateway_profile_row(row: &sqlx::postgres::PgRow) -> Result<ModelGatewayProfile> {
    Ok(ModelGatewayProfile {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        profile_id: row.get("profile_id"),
        display_name: row.get("display_name"),
        lane: row.get("lane"),
        provider_id: row.get("provider_id"),
        model_id: row.get("model_id"),
        base_url: row.get("base_url"),
        api_path: row.get("api_path"),
        wire_api: row.get("wire_api"),
        auth_mode: row.get("auth_mode"),
        auth_env_key_name: row.get("auth_env_key_name"),
        recommended_preset: row.get("recommended_preset"),
        max_concurrency: row.get("max_concurrency"),
        rpm_limit: row.get("rpm_limit"),
        tpm_limit: row.get("tpm_limit"),
        timeout_ms: row.get("timeout_ms"),
        priority: row.get("priority"),
        enabled: row.get("enabled"),
        capabilities: row.get("capabilities"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_asset_library_row(row: &sqlx::postgres::PgRow) -> Result<AssetLibraryRecord> {
    let dataset_count = row.get::<i64, _>("dataset_count").max(0) as usize;

    Ok(AssetLibraryRecord {
        id: row.get::<Uuid, _>("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        external_id: row.get("external_id"),
        name: row.get("name"),
        domain: row.get("domain"),
        description: row.get("description"),
        visibility: row.get("visibility"),
        metadata: row.get("metadata"),
        dataset_count,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_asset_library_dataset_membership_row(
    row: &sqlx::postgres::PgRow,
) -> AssetLibraryDatasetMembershipRecord {
    AssetLibraryDatasetMembershipRecord {
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        asset_library_id: row.get::<Uuid, _>("asset_library_id"),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        role: row.get("role"),
        priority: row.get("priority"),
        created_at: row.get("created_at"),
    }
}

fn map_asset_item_row(row: &sqlx::postgres::PgRow) -> Result<AssetItemRecord> {
    let profile_count = row.get::<i64, _>("profile_count").max(0) as usize;

    Ok(AssetItemRecord {
        id: row.get::<Uuid, _>("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        asset_library_id: row.get("asset_library_id"),
        collection_id: row.get("collection_id"),
        external_id: row.get("external_id"),
        title: row.get("title"),
        asset_kind: row.get("asset_kind"),
        source_kind: row.get("source_kind"),
        source_id: row.get("source_id"),
        content_type: row.get("content_type"),
        object_key: row.get("object_key"),
        metadata: row.get("metadata"),
        profile_count,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_dataset_asset_membership_row(row: &sqlx::postgres::PgRow) -> DatasetAssetMembershipRecord {
    DatasetAssetMembershipRecord {
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        asset_id: row.get("asset_id"),
        membership_kind: row.get("membership_kind"),
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
    }
}

fn map_asset_parse_run_row(row: &sqlx::postgres::PgRow) -> AssetParseRunRecord {
    AssetParseRunRecord {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        asset_id: row.get("asset_id"),
        parser_name: row.get("parser_name"),
        parser_version: row.get("parser_version"),
        status: row.get("status"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        error_code: row.get("error_code"),
        error_message: row.get("error_message"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_asset_profile_row(row: &sqlx::postgres::PgRow) -> AssetProfileRecord {
    AssetProfileRecord {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        asset_id: row.get("asset_id"),
        profile_kind: row.get("profile_kind"),
        profile_version: row.get("profile_version"),
        attributes: row.get("attributes"),
        embedding_status: row.get("embedding_status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn document_asset_kind(document: &Document) -> String {
    let content_type = normalized_content_type(&document.content_type);
    let title = document.title.to_ascii_lowercase();
    if content_type.starts_with("image/") {
        "image".to_string()
    } else if content_type.starts_with("video/") {
        "video".to_string()
    } else if content_type.contains("presentation")
        || content_type.contains("powerpoint")
        || title.ends_with(".ppt")
        || title.ends_with(".pptx")
    {
        "presentation".to_string()
    } else {
        "document".to_string()
    }
}

fn document_asset_profile_kind(document: &Document) -> String {
    match document_asset_kind(document).as_str() {
        "image" => "image_semantic".to_string(),
        "video" => "video_summary".to_string(),
        "presentation" => "presentation_outline".to_string(),
        _ => "document_parse".to_string(),
    }
}

fn document_asset_metadata(document: &Document) -> Value {
    let mut metadata = Map::new();
    metadata.insert(
        "document_id".to_string(),
        Value::String(document.id.0.to_string()),
    );
    metadata.insert(
        "dataset_id".to_string(),
        Value::String(document.dataset_id.0.to_string()),
    );
    metadata.insert(
        "content_type".to_string(),
        Value::String(document.content_type.clone()),
    );
    metadata.insert(
        "lifecycle".to_string(),
        Value::String(document.lifecycle.as_str().to_string()),
    );
    Value::Object(metadata)
}

fn document_asset_profile_attributes(document: &Document, chunks: &[DocumentChunk]) -> Value {
    let mut attributes = Map::new();
    attributes.insert("title".to_string(), Value::String(document.title.clone()));
    attributes.insert(
        "content_type".to_string(),
        Value::String(document.content_type.clone()),
    );
    attributes.insert(
        "lifecycle".to_string(),
        Value::String(document.lifecycle.as_str().to_string()),
    );
    attributes.insert(
        "chunk_count".to_string(),
        Value::Number(serde_json::Number::from(chunks.len() as u64)),
    );
    attributes.insert(
        "token_count".to_string(),
        Value::Number(serde_json::Number::from(
            chunks
                .iter()
                .map(|chunk| chunk.token_count.max(0) as u64)
                .sum::<u64>(),
        )),
    );

    let document_metadata = Value::Object(Map::from_iter(document.metadata.clone()));
    for key in ["parse_status", "parse_quality_status", "parse_method"] {
        if let Some(value) = compact_metadata_string(&document_metadata, key) {
            attributes.insert(key.to_string(), Value::String(value));
        }
    }
    if let Some(media) = document_metadata
        .get("parse_metadata")
        .and_then(|value| value.get("media"))
        .or_else(|| document_metadata.get("media"))
        .cloned()
    {
        attributes.insert("media".to_string(), compact_metadata_value(media, 2000));
    }
    apply_document_multimodal_profile_attributes(
        &mut attributes,
        document_asset_kind(document).as_str(),
        &document_metadata,
        chunks,
    );
    if let Some(summary) = document_profile_summary(&document_metadata, chunks) {
        attributes.insert("summary".to_string(), Value::String(summary));
    }
    let noun_terms = document_profile_noun_terms(&document_metadata, chunks);
    if !noun_terms.is_empty() {
        attributes.insert(
            "noun_terms".to_string(),
            Value::Array(noun_terms.into_iter().map(Value::String).collect()),
        );
    }

    Value::Object(attributes)
}

fn document_profile_summary(document_metadata: &Value, chunks: &[DocumentChunk]) -> Option<String> {
    for path in [
        &["summary"][..],
        &["description"][..],
        &["caption"][..],
        &["vlm", "payload", "summary"][..],
        &["vlm", "payload", "visualSummary"][..],
        &["parse_metadata", "vlm", "payload", "summary"][..],
        &["parse_metadata", "vlm", "payload", "visualSummary"][..],
        &["parse_metadata", "summary"][..],
        &["parse_metadata", "media", "summary"][..],
        &["media", "summary"][..],
        &["media", "transcript_summary"][..],
    ] {
        if let Some(value) = metadata_path_string(document_metadata, path) {
            return Some(limit_chars(&value, 600));
        }
    }
    let summary = chunks
        .iter()
        .take(3)
        .map(|chunk| normalize_inline_text(&chunk.content))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if summary.is_empty() {
        None
    } else {
        Some(limit_chars(&summary, 600))
    }
}

fn document_profile_noun_terms(document_metadata: &Value, chunks: &[DocumentChunk]) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for key in [
        "noun_terms",
        "nounTermHints",
        "material_hints",
        "materialHints",
        "section_title_hints",
        "sectionTitleHints",
        "document_title_hints",
        "documentTitleHints",
        "keywords",
        "tags",
        "topicTags",
    ] {
        collect_profile_terms(document_metadata.get(key), &mut terms);
    }
    for path in [
        &["vlm", "payload", "topicTags"][..],
        &["parse_metadata", "vlm", "payload", "topicTags"][..],
        &["vlm", "payload", "tableLikeSignals"][..],
        &["parse_metadata", "vlm", "payload", "tableLikeSignals"][..],
    ] {
        collect_profile_terms(metadata_path_value(document_metadata, path), &mut terms);
    }
    if let Some(payload) = document_vlm_payload_metadata(document_metadata) {
        collect_profile_object_texts(
            payload.get("entities"),
            &["name", "text", "type"],
            &mut terms,
        );
        collect_profile_object_texts(
            payload.get("fieldCandidates"),
            &["key", "evidenceText"],
            &mut terms,
        );
    }
    if let Some(media) = document_profile_media_metadata(document_metadata, chunks) {
        collect_profile_object_texts(
            media.get("transcript_segments"),
            &["text", "summary"],
            &mut terms,
        );
        collect_profile_object_texts(media.get("scenes"), &["summary", "label"], &mut terms);
        collect_profile_object_texts(
            media.get("keyframe_ocr_snippets"),
            &["text", "content"],
            &mut terms,
        );
    }
    for chunk in chunks.iter().take(8) {
        let chunk_metadata = Value::Object(Map::from_iter(chunk.metadata.clone()));
        for key in [
            "noun_terms",
            "nounTermHints",
            "keywords",
            "tags",
            "section_title",
            "section_title_hints",
            "sectionTitleHints",
        ] {
            collect_profile_terms(chunk_metadata.get(key), &mut terms);
        }
    }
    terms.into_iter().take(48).collect()
}

fn apply_document_multimodal_profile_attributes(
    attributes: &mut Map<String, Value>,
    asset_kind: &str,
    document_metadata: &Value,
    chunks: &[DocumentChunk],
) {
    match asset_kind {
        "image" => apply_image_profile_attributes(attributes, document_metadata, chunks),
        "video" => apply_video_profile_attributes(attributes, document_metadata, chunks),
        "presentation" => apply_presentation_profile_attributes(attributes, chunks),
        _ => {}
    }
}

fn apply_image_profile_attributes(
    attributes: &mut Map<String, Value>,
    document_metadata: &Value,
    chunks: &[DocumentChunk],
) {
    let Some(payload) = document_vlm_payload_metadata(document_metadata) else {
        if let Some(ocr_text) = document_chunk_text_sample(chunks, 600) {
            attributes.insert("ocr_text".to_string(), Value::String(ocr_text));
        }
        return;
    };

    insert_metadata_path_string(
        attributes,
        "visual_summary",
        payload,
        &["visualSummary"],
        600,
    );
    insert_metadata_path_string(attributes, "document_kind", payload, &["documentKind"], 120);
    insert_metadata_path_string(attributes, "layout_type", payload, &["layoutType"], 120);
    insert_metadata_path_string(attributes, "risk_level", payload, &["riskLevel"], 80);
    insert_metadata_path_string(attributes, "ocr_text", payload, &["transcribedText"], 1000);
    if let Some(value) = payload.get("chartOrTableDetected").and_then(Value::as_bool) {
        attributes.insert("chart_or_table_detected".to_string(), Value::Bool(value));
    }
    if let Some(tags) = compact_string_array(payload.get("topicTags"), 16, 80) {
        attributes.insert("tags".to_string(), tags);
    }
    if let Some(signals) = compact_string_array(payload.get("tableLikeSignals"), 12, 120) {
        attributes.insert("table_like_signals".to_string(), signals);
    }
    if let Some(entities) =
        compact_object_text_rows(payload.get("entities"), &["name", "text", "type"], 12, 120)
    {
        attributes.insert("entities".to_string(), entities);
    }
    if let Some(fields) = compact_object_text_rows(
        payload.get("fieldCandidates"),
        &["key", "value", "source", "evidenceText"],
        12,
        120,
    ) {
        attributes.insert("field_candidates".to_string(), fields);
    }
}

fn apply_video_profile_attributes(
    attributes: &mut Map<String, Value>,
    document_metadata: &Value,
    chunks: &[DocumentChunk],
) {
    let Some(media) = document_profile_media_metadata(document_metadata, chunks) else {
        return;
    };
    insert_metadata_path_string(attributes, "media_kind", &media, &["kind"], 80);
    insert_metadata_path_string(
        attributes,
        "media_parse_status",
        &media,
        &["parse_status"],
        80,
    );
    for (target_key, media_key) in [
        ("transcript_segment_count", "transcript_segments"),
        ("scene_count", "scenes"),
        ("keyframe_ocr_snippet_count", "keyframe_ocr_snippets"),
    ] {
        attributes.insert(
            target_key.to_string(),
            Value::Number(serde_json::Number::from(
                media_array_len(&media, media_key) as u64
            )),
        );
    }
    if let Some(summary) = compact_object_text_join(
        media.get("transcript_segments"),
        &["text", "summary"],
        4,
        800,
    ) {
        attributes.insert("transcript_summary".to_string(), Value::String(summary));
    }
    if let Some(ocr_text) = compact_object_text_join(
        media.get("keyframe_ocr_snippets"),
        &["text", "content"],
        6,
        800,
    ) {
        attributes.insert("ocr_text".to_string(), Value::String(ocr_text));
    }
    if let Some(scene_summaries) =
        compact_object_text_rows(media.get("scenes"), &["summary", "label"], 8, 160)
    {
        attributes.insert("scene_summaries".to_string(), scene_summaries);
    }
    if let Some(provider_evidence) = compact_object_text_rows(
        media
            .get("provider_evidence")
            .or_else(|| media.get("provider_capabilities")),
        &["capability", "status", "detail"],
        8,
        160,
    ) {
        attributes.insert("provider_evidence".to_string(), provider_evidence);
    }
}

fn apply_presentation_profile_attributes(
    attributes: &mut Map<String, Value>,
    chunks: &[DocumentChunk],
) {
    let outline = presentation_outline_from_chunks(chunks, 16);
    if !outline.is_empty() {
        attributes.insert(
            "outline".to_string(),
            Value::Array(outline.iter().cloned().map(Value::String).collect()),
        );
        attributes.insert(
            "slide_count_estimate".to_string(),
            Value::Number(serde_json::Number::from(outline.len() as u64)),
        );
    }
    let samples = chunks
        .iter()
        .take(8)
        .map(|chunk| normalize_inline_text(&chunk.content))
        .filter(|text| !text.is_empty())
        .map(|text| limit_chars(&text, 260))
        .collect::<Vec<_>>();
    if !samples.is_empty() {
        attributes.insert(
            "slide_text_samples".to_string(),
            Value::Array(samples.into_iter().map(Value::String).collect()),
        );
    }
}

fn collect_profile_terms(value: Option<&Value>, terms: &mut BTreeSet<String>) {
    match value {
        Some(Value::String(text)) => {
            let text = normalize_inline_text(text);
            if !text.is_empty() {
                terms.insert(limit_chars(&text, 80));
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(text) = item.as_str() {
                    let text = normalize_inline_text(text);
                    if !text.is_empty() {
                        terms.insert(limit_chars(&text, 80));
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect_profile_object_texts(
    value: Option<&Value>,
    keys: &[&str],
    terms: &mut BTreeSet<String>,
) {
    match value {
        Some(Value::Array(items)) => {
            for item in items.iter().take(16) {
                for key in keys {
                    collect_profile_terms(item.get(*key), terms);
                }
            }
        }
        Some(Value::Object(object)) => {
            for item in object.values().take(16) {
                if item.is_object() {
                    for key in keys {
                        collect_profile_terms(item.get(*key), terms);
                    }
                } else {
                    collect_profile_terms(Some(item), terms);
                }
            }
        }
        _ => {}
    }
}

fn document_vlm_payload_metadata(document_metadata: &Value) -> Option<&Value> {
    document_metadata
        .pointer("/vlm/payload")
        .or_else(|| document_metadata.pointer("/parse_metadata/vlm/payload"))
}

fn document_profile_media_metadata(
    document_metadata: &Value,
    chunks: &[DocumentChunk],
) -> Option<Value> {
    document_metadata
        .pointer("/parse_metadata/media")
        .or_else(|| document_metadata.get("media"))
        .cloned()
        .or_else(|| {
            chunks.iter().find_map(|chunk| {
                let chunk_metadata = Value::Object(Map::from_iter(chunk.metadata.clone()));
                chunk_metadata
                    .pointer("/parse_metadata/media")
                    .or_else(|| chunk_metadata.get("media"))
                    .cloned()
            })
        })
}

fn insert_metadata_path_string(
    attributes: &mut Map<String, Value>,
    target_key: &str,
    value: &Value,
    path: &[&str],
    max_chars: usize,
) {
    if let Some(text) = metadata_path_string(value, path) {
        attributes.insert(
            target_key.to_string(),
            Value::String(limit_chars(&text, max_chars)),
        );
    }
}

fn document_chunk_text_sample(chunks: &[DocumentChunk], max_chars: usize) -> Option<String> {
    let text = chunks
        .iter()
        .take(4)
        .map(|chunk| normalize_inline_text(&chunk.content))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then(|| limit_chars(&text, max_chars))
}

fn compact_string_array(value: Option<&Value>, limit: usize, max_chars: usize) -> Option<Value> {
    let values = match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(normalize_inline_text)
            .filter(|text| !text.is_empty())
            .map(|text| limit_chars(&text, max_chars))
            .take(limit)
            .collect::<Vec<_>>(),
        Some(Value::String(text)) => {
            let text = normalize_inline_text(text);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![limit_chars(&text, max_chars)]
            }
        }
        _ => Vec::new(),
    };
    (!values.is_empty()).then(|| Value::Array(values.into_iter().map(Value::String).collect()))
}

fn compact_object_text_rows(
    value: Option<&Value>,
    keys: &[&str],
    limit: usize,
    max_chars: usize,
) -> Option<Value> {
    let mut rows = Vec::new();
    match value {
        Some(Value::Array(items)) => {
            for item in items.iter().take(limit) {
                if let Some(row) = compact_object_text_row(item, keys, max_chars) {
                    rows.push(row);
                }
            }
        }
        Some(Value::Object(object)) => {
            for (key, item) in object.iter().take(limit) {
                if let Some(mut row) = compact_object_text_row(item, keys, max_chars) {
                    if let Some(row_object) = row.as_object_mut() {
                        row_object
                            .entry("key".to_string())
                            .or_insert_with(|| Value::String(limit_chars(key, max_chars)));
                    }
                    rows.push(row);
                }
            }
        }
        _ => {}
    }
    (!rows.is_empty()).then(|| Value::Array(rows))
}

fn compact_object_text_row(value: &Value, keys: &[&str], max_chars: usize) -> Option<Value> {
    let object = value.as_object()?;
    let mut row = Map::new();
    for key in keys {
        if let Some(text) = object
            .get(*key)
            .and_then(compact_metadata_scalar_text)
            .map(|text| limit_chars(&text, max_chars))
            .filter(|text| !text.is_empty())
        {
            row.insert((*key).to_string(), Value::String(text));
        }
    }
    (!row.is_empty()).then(|| Value::Object(row))
}

fn compact_metadata_scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let text = normalize_inline_text(text);
            (!text.is_empty()).then_some(text)
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn compact_object_text_join(
    value: Option<&Value>,
    keys: &[&str],
    limit: usize,
    max_chars: usize,
) -> Option<String> {
    let texts = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(limit)
        .filter_map(|item| keys.iter().find_map(|key| item.get(*key)))
        .filter_map(compact_metadata_scalar_text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    let text = texts.join("\n");
    (!text.is_empty()).then(|| limit_chars(&text, max_chars))
}

fn media_array_len(media: &Value, key: &str) -> usize {
    media.get(key).and_then(Value::as_array).map_or(0, Vec::len)
}

fn presentation_outline_from_chunks(chunks: &[DocumentChunk], limit: usize) -> Vec<String> {
    let mut outline = Vec::new();
    for chunk in chunks.iter().take(32) {
        let metadata = Value::Object(Map::from_iter(chunk.metadata.clone()));
        for key in ["section_title", "sectionTitle"] {
            push_profile_terms(metadata.get(key), &mut outline, limit);
        }
        for key in ["section_title_hints", "sectionTitleHints"] {
            push_profile_terms(metadata.get(key), &mut outline, limit);
        }
        if outline.len() < limit {
            if let Some(line) = chunk
                .content
                .lines()
                .map(normalize_inline_text)
                .find(|line| !line.is_empty())
            {
                push_unique_profile_term(&mut outline, limit_chars(&line, 100), limit);
            }
        }
    }
    outline
}

fn push_profile_terms(value: Option<&Value>, target: &mut Vec<String>, limit: usize) {
    match value {
        Some(Value::String(text)) => {
            push_unique_profile_term(
                target,
                limit_chars(&normalize_inline_text(text), 100),
                limit,
            );
        }
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(text) = item.as_str() {
                    push_unique_profile_term(
                        target,
                        limit_chars(&normalize_inline_text(text), 100),
                        limit,
                    );
                }
            }
        }
        _ => {}
    }
}

fn push_unique_profile_term(target: &mut Vec<String>, value: String, limit: usize) {
    if value.is_empty() || target.len() >= limit || target.iter().any(|existing| existing == &value)
    {
        return;
    }
    target.push(value);
}

fn compact_metadata_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(normalize_inline_text)
        .filter(|text| !text.is_empty())
}

fn metadata_path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn metadata_path_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .map(normalize_inline_text)
        .filter(|text| !text.is_empty())
}

fn compact_metadata_value(value: Value, max_chars: usize) -> Value {
    match value {
        Value::String(text) => Value::String(limit_chars(&normalize_inline_text(&text), max_chars)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .take(20)
                .map(|item| compact_metadata_value(item, max_chars / 2))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .take(20)
                .map(|(key, value)| (key, compact_metadata_value(value, max_chars / 2)))
                .collect(),
        ),
        other => other,
    }
}

fn normalized_content_type(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase()
}

fn normalize_inline_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn limit_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn map_secret_binding_row(row: &sqlx::postgres::PgRow) -> Result<SecretBinding> {
    let scope_level = row.get::<String, _>("scope_level");

    Ok(SecretBinding {
        id: SecretBindingId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        document_id: row.get::<Option<Uuid>, _>("document_id").map(DocumentId),
        scope_level: SecretScopeLevel::from_str(&scope_level)
            .ok_or_else(|| anyhow!("unknown secret scope level: {scope_level}"))?,
        provider_key: row.get("provider_key"),
        cipher_text: row.get("cipher_text"),
        fingerprint: row.get("fingerprint"),
        created_at: row.get("created_at"),
    })
}

fn map_user_row(row: &sqlx::postgres::PgRow) -> Result<User> {
    let roles = row.get::<Value, _>("roles");

    Ok(User {
        id: UserId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        email: row.get("email"),
        display_name: row.get("display_name"),
        roles: serde_json::from_value(roles)?,
        created_at: row.get("created_at"),
    })
}

fn map_user_session_row(row: &sqlx::postgres::PgRow) -> Result<UserSession> {
    let auth_method = row.get::<String, _>("auth_method");

    Ok(UserSession {
        id: UserSessionId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        user_id: UserId(row.get::<Uuid, _>("user_id")),
        device_fingerprint: row.get("device_fingerprint"),
        session_token_hash: row.get("session_token_hash"),
        auth_method: AuthSessionMethod::from_str(&auth_method)
            .ok_or_else(|| anyhow!("unknown auth session method: {auth_method}"))?,
        created_at: row.get("created_at"),
        last_seen_at: row.get("last_seen_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
    })
}

fn map_email_verification_challenge_row(
    row: &sqlx::postgres::PgRow,
) -> Result<EmailVerificationChallenge> {
    let purpose = row.get::<String, _>("purpose");

    Ok(EmailVerificationChallenge {
        id: EmailVerificationChallengeId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        email_normalized: row.get("email_normalized"),
        purpose: AuthChallengePurpose::from_str(&purpose)
            .ok_or_else(|| anyhow!("unknown auth challenge purpose: {purpose}"))?,
        code_hash: row.get("code_hash"),
        attempt_count: row.get("attempt_count"),
        max_attempts: row.get("max_attempts"),
        expires_at: row.get("expires_at"),
        consumed_at: row.get("consumed_at"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    })
}

fn map_document_chunk_row(row: &sqlx::postgres::PgRow) -> Result<DocumentChunk> {
    let state = row.get::<String, _>("state");
    let metadata = row.get::<Value, _>("metadata");

    Ok(DocumentChunk {
        id: DocumentChunkId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        document_id: DocumentId(row.get::<Uuid, _>("document_id")),
        chunk_index: row.get("chunk_index"),
        content: row.get("content"),
        token_count: row.get("token_count"),
        state: DocumentChunkState::from_str(&state)
            .ok_or_else(|| anyhow!("unknown document chunk state: {state}"))?,
        metadata: json_object_to_btree_map(metadata)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_document_fact_row(row: &sqlx::postgres::PgRow) -> Result<DocumentFact> {
    Ok(DocumentFact {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        document_id: DocumentId(row.get::<Uuid, _>("document_id")),
        fact_type: row.get("fact_type"),
        name: row.get("name"),
        normalized_name: row.get("normalized_name"),
        value_text: row.get("value_text"),
        value_number: row.get("value_number"),
        value_date: row.get("value_date"),
        attributes: row.get("attributes"),
        confidence: row.get("confidence"),
        source_kind: row.get("source_kind"),
        source_locator: row.get("source_locator"),
        source_chunk_id: row
            .get::<Option<Uuid>, _>("source_chunk_id")
            .map(DocumentChunkId),
        parse_version: row.get("parse_version"),
        created_at: row.get("created_at"),
    })
}

fn map_dataset_fact_snapshot_row(row: &sqlx::postgres::PgRow) -> Result<DatasetFactSnapshot> {
    Ok(DatasetFactSnapshot {
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        snapshot_kind: row.get("snapshot_kind"),
        snapshot_key: row.get("snapshot_key"),
        snapshot_manifest: row.get("snapshot_manifest"),
        source_fact_count: row.get("source_fact_count"),
        source_document_count: row.get("source_document_count"),
        created_at: row.get("created_at"),
    })
}

fn map_dataset_semantic_snapshot_row(
    row: &sqlx::postgres::PgRow,
) -> Result<DatasetSemanticSnapshot> {
    Ok(DatasetSemanticSnapshot {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        schema_version: row.get("schema_version"),
        generation_version: row.get("generation_version"),
        source_fingerprint: row.get("source_fingerprint"),
        status: row.get("status"),
        manifest: row.get("manifest"),
        source_document_count: row.get("source_document_count"),
        source_asset_count: row.get("source_asset_count"),
        source_record_count: row.get("source_record_count"),
        node_count: row.get("node_count"),
        edge_count: row.get("edge_count"),
        failure_code: row.get("failure_code"),
        generated_at: row.get("generated_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_dataset_semantic_link_snapshot_row(
    row: &sqlx::postgres::PgRow,
) -> Result<DatasetSemanticLinkSnapshot> {
    Ok(DatasetSemanticLinkSnapshot {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        left_dataset_id: DatasetId(row.get::<Uuid, _>("left_dataset_id")),
        right_dataset_id: DatasetId(row.get::<Uuid, _>("right_dataset_id")),
        left_snapshot_id: row.get("left_snapshot_id"),
        right_snapshot_id: row.get("right_snapshot_id"),
        schema_version: row.get("schema_version"),
        generation_version: row.get("generation_version"),
        source_fingerprint: row.get("source_fingerprint"),
        status: row.get("status"),
        manifest: row.get("manifest"),
        node_count: row.get("node_count"),
        edge_count: row.get("edge_count"),
        failure_code: row.get("failure_code"),
        generated_at: row.get("generated_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_dataset_semantic_link_run_row(
    row: &sqlx::postgres::PgRow,
) -> Result<DatasetSemanticLinkRun> {
    Ok(DatasetSemanticLinkRun {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        left_dataset_id: DatasetId(row.get::<Uuid, _>("left_dataset_id")),
        right_dataset_id: DatasetId(row.get::<Uuid, _>("right_dataset_id")),
        left_snapshot_id: row.get("left_snapshot_id"),
        right_snapshot_id: row.get("right_snapshot_id"),
        generation_version: row.get("generation_version"),
        source_fingerprint: row.get("source_fingerprint"),
        status: row.get("status"),
        priority: row.get("priority"),
        attempt_count: row.get("attempt_count"),
        max_attempts: row.get("max_attempts"),
        available_at: row.get("available_at"),
        claimed_at: row.get("claimed_at"),
        finished_at: row.get("finished_at"),
        failure_code: row.get("failure_code"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_semantic_dictionary_entry_row(
    row: &sqlx::postgres::PgRow,
) -> Result<SemanticDictionaryEntry> {
    Ok(SemanticDictionaryEntry {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        source_kind: row.get("source_kind"),
        source_system_key: row.get("source_system_key"),
        source_object_key: row.get("source_object_key"),
        raw_field_key: row.get("raw_field_key"),
        display_name: row.get("display_name"),
        description: row.get("description"),
        semantic_role: row.get("semantic_role"),
        value_type: row.get("value_type"),
        status: row.get("status"),
        confidence: row.get("confidence"),
        created_by_user_id: row.get::<Option<Uuid>, _>("created_by_user_id").map(UserId),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_report_plan_row(row: &sqlx::postgres::PgRow) -> Result<ReportPlan> {
    let status = row.get::<String, _>("status");

    Ok(ReportPlan {
        id: ReportPlanId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        title: row.get("title"),
        objective: row.get("objective"),
        status: ReportPlanStatus::from_str(&status)
            .ok_or_else(|| anyhow!("unknown report plan status: {status}"))?,
        theme_key: row.get("theme_key"),
        current_ast_version_id: row
            .get::<Option<Uuid>, _>("current_ast_version_id")
            .map(ReportPlanAstVersionId),
        modules: Vec::new(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_report_plan_ast_version_row(row: &sqlx::postgres::PgRow) -> Result<ReportPlanAstVersion> {
    Ok(ReportPlanAstVersion {
        id: ReportPlanAstVersionId(row.get::<Uuid, _>("id")),
        plan_id: ReportPlanId(row.get::<Uuid, _>("plan_id")),
        version_no: row.get("version_no"),
        ast: row.get("ast"),
        created_at: row.get("created_at"),
    })
}

fn map_report_render_output_row(row: &sqlx::postgres::PgRow) -> Result<ReportRenderOutput> {
    let surface = row.get::<String, _>("surface");
    let status = row.get::<String, _>("status");

    Ok(ReportRenderOutput {
        id: ReportRenderOutputId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        plan_id: ReportPlanId(row.get::<Uuid, _>("plan_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        ast_version_id: ReportPlanAstVersionId(row.get::<Uuid, _>("ast_version_id")),
        surface: domain_model::PublishedSurface::from_str(&surface)
            .ok_or_else(|| anyhow!("unknown report render surface: {surface}"))?,
        status: ReportRenderOutputStatus::from_str(&status)
            .ok_or_else(|| anyhow!("unknown report render output status: {status}"))?,
        asset_manifest: row.get("asset_manifest"),
        created_at: row.get("created_at"),
    })
}

fn map_published_report_row(row: &sqlx::postgres::PgRow) -> Result<PublishedReport> {
    Ok(PublishedReport {
        id: PublishedReportId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        plan_id: ReportPlanId(row.get::<Uuid, _>("plan_id")),
        slug: row.get("slug"),
        current_version_id: row
            .get::<Option<Uuid>, _>("current_version_id")
            .map(PublishedReportVersionId),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_published_report_version_row(row: &sqlx::postgres::PgRow) -> Result<PublishedReportVersion> {
    let surface = row.get::<String, _>("surface");

    Ok(PublishedReportVersion {
        id: PublishedReportVersionId(row.get::<Uuid, _>("id")),
        report_id: PublishedReportId(row.get::<Uuid, _>("report_id")),
        version_no: row.get("version_no"),
        surface: domain_model::PublishedSurface::from_str(&surface)
            .ok_or_else(|| anyhow!("unknown published report surface: {surface}"))?,
        asset_manifest: row.get("asset_manifest"),
        created_at: row.get("created_at"),
    })
}

fn map_published_video_ppt_package_row(
    row: &sqlx::postgres::PgRow,
) -> Result<PublishedVideoPptPackage> {
    Ok(PublishedVideoPptPackage {
        id: PublishedVideoPptPackageId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        assistant_run_id: AssistantRunId(row.get::<Uuid, _>("assistant_run_id")),
        document_id: DocumentId(row.get::<Uuid, _>("document_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        package_key: row.get("package_key"),
        current_version_id: row
            .get::<Option<Uuid>, _>("current_version_id")
            .map(PublishedVideoPptVersionId),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_published_video_ppt_version_row(
    row: &sqlx::postgres::PgRow,
) -> Result<PublishedVideoPptVersion> {
    Ok(PublishedVideoPptVersion {
        id: PublishedVideoPptVersionId(row.get::<Uuid, _>("id")),
        package_id: PublishedVideoPptPackageId(row.get::<Uuid, _>("package_id")),
        version_no: row.get("version_no"),
        version_fingerprint: row.get("version_fingerprint"),
        lifecycle_state: row.get("lifecycle_state"),
        artifact_manifest: row.get("artifact_manifest"),
        created_at: row.get("created_at"),
    })
}

fn map_memory_directory_row(row: &sqlx::postgres::PgRow) -> Result<MemoryDirectory> {
    Ok(MemoryDirectory {
        id: MemoryDirectoryId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        source_document_ids: row
            .get::<Vec<Uuid>, _>("source_document_ids")
            .into_iter()
            .map(DocumentId)
            .collect(),
        version_no: row.get("version_no"),
        directory_nodes: row.get("directory_nodes"),
        refreshed_chunks: row.get("refreshed_chunks"),
        directory_manifest: row.get("directory_manifest"),
        created_at: row.get("created_at"),
    })
}

fn map_retrieval_evidence_row(row: &sqlx::postgres::PgRow) -> Result<RetrievalEvidence> {
    Ok(RetrievalEvidence {
        id: RetrievalEvidenceId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        document_id: DocumentId(row.get::<Uuid, _>("document_id")),
        document_chunk_id: DocumentChunkId(row.get::<Uuid, _>("document_chunk_id")),
        chunk_index: row.get("chunk_index"),
        source_locator: row.get("source_locator"),
        content_excerpt: row.get("content_excerpt"),
        summary: row.get("summary"),
        payload_filter_key: row.get("payload_filter_key"),
        embedding_model: row.get("embedding_model"),
        recall_score: row.get("recall_score"),
        evidence_manifest: row.get("evidence_manifest"),
        created_at: row.get("created_at"),
    })
}

fn map_asset_retrieval_evidence_row(row: &sqlx::postgres::PgRow) -> AssetRetrievalEvidenceRecord {
    AssetRetrievalEvidenceRecord {
        id: row.get("id"),
        tenant_id: TenantId(row.get("tenant_id")),
        dataset_id: DatasetId(row.get("dataset_id")),
        asset_id: row.get("asset_id"),
        asset_profile_id: row.get("asset_profile_id"),
        profile_kind: row.get("profile_kind"),
        profile_version: row.get("profile_version"),
        materialized_text: row.get("materialized_text"),
        safe_metadata: row.get("safe_metadata"),
        content_hash: row.get("content_hash"),
        search_terms: row.get("search_terms"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_dataset_output_row(row: &sqlx::postgres::PgRow) -> Result<DatasetOutput> {
    Ok(DatasetOutput {
        id: DatasetOutputId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        prompt: row.get("prompt"),
        output_text: row.get("output_text"),
        memory_directory_id: row
            .get::<Option<Uuid>, _>("memory_directory_id")
            .map(MemoryDirectoryId),
        retrieval_evidence_ids: row
            .get::<Vec<Uuid>, _>("retrieval_evidence_ids")
            .into_iter()
            .map(RetrievalEvidenceId)
            .collect(),
        output_manifest: row.get("output_manifest"),
        created_at: row.get("created_at"),
    })
}

fn map_assistant_run_row(row: &sqlx::postgres::PgRow) -> Result<AssistantRun> {
    Ok(AssistantRun {
        id: AssistantRunId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        user_id: row.get::<Option<Uuid>, _>("user_id").map(UserId),
        local_thread_id: row.get("local_thread_id"),
        user_prompt: row.get("user_prompt"),
        startup_briefing: row.get("startup_briefing"),
        selected_scope: row.get("selected_scope"),
        scope_candidates: row.get("scope_candidates"),
        context_policy: row.get("context_policy"),
        evidence_state: row.get("evidence_state"),
        service_lane: row.get("service_lane"),
        execution_trail: row.get("execution_trail"),
        output_artifacts: row.get("output_artifacts"),
        runtime_manifest: row.get("runtime_manifest"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_assistant_run_event_row(row: &sqlx::postgres::PgRow) -> Result<AssistantRunEvent> {
    Ok(AssistantRunEvent {
        id: AssistantRunEventId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        run_id: AssistantRunId(row.get::<Uuid, _>("run_id")),
        sequence_no: row.get("sequence_no"),
        event_name: row.get("event_name"),
        payload: row.get("payload"),
        created_at: row.get("created_at"),
    })
}

fn is_assistant_run_event_sequence_unique_violation(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(database_error) = error else {
        return false;
    };
    database_error.code().as_deref() == Some("23505")
        && database_error.constraint() == Some("assistant_run_events_run_id_sequence_no_key")
}

fn map_conversation_memory_item_row(row: &sqlx::postgres::PgRow) -> Result<ConversationMemoryItem> {
    let role = row.get::<String, _>("role");
    Ok(ConversationMemoryItem {
        id: ConversationMemoryItemId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        user_id: row.get::<Option<Uuid>, _>("user_id").map(UserId),
        local_thread_id: row.get("local_thread_id"),
        role: ChatMessageRole::from_str(&role)
            .ok_or_else(|| anyhow!("unknown conversation memory role: {role}"))?,
        item_kind: row.get("item_kind"),
        summary: row.get("summary"),
        source_message_refs: row.get("source_message_refs"),
        artifact_refs: row.get("artifact_refs"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_html_artifact_row(row: &sqlx::postgres::PgRow) -> Result<HtmlArtifact> {
    Ok(HtmlArtifact {
        id: row.get("id"),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        assistant_run_id: row
            .get::<Option<Uuid>, _>("assistant_run_id")
            .map(AssistantRunId),
        local_thread_id: row.get("local_thread_id"),
        source_type: row.get("source_type"),
        template_id: row.get("template_id"),
        interaction_mode: row.get("interaction_mode"),
        manifest: row.get("manifest"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_static_page_draft_row(row: &sqlx::postgres::PgRow) -> Result<StaticPageDraft> {
    let status = row.get::<String, _>("status");
    Ok(StaticPageDraft {
        id: StaticPageDraftId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        assistant_run_id: AssistantRunId(row.get::<Uuid, _>("assistant_run_id")),
        title: row.get("title"),
        status: StaticPageDraftStatus::from_str(&status)
            .ok_or_else(|| anyhow!("unknown static page draft status: {status}"))?,
        selected_scope: row.get("selected_scope"),
        visibility_snapshot: row.get("visibility_snapshot"),
        source_refs: row.get("source_refs"),
        draft_payload: row.get("draft_payload"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_static_page_image_job_row(row: &sqlx::postgres::PgRow) -> Result<StaticPageImageJob> {
    let status = row.get::<String, _>("status");
    Ok(StaticPageImageJob {
        id: StaticPageImageJobId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        draft_id: StaticPageDraftId(row.get::<Uuid, _>("draft_id")),
        assistant_run_id: AssistantRunId(row.get::<Uuid, _>("assistant_run_id")),
        status: StaticPageImageJobStatus::from_str(&status)
            .ok_or_else(|| anyhow!("unknown static page image job status: {status}"))?,
        queue_position: row.get("queue_position"),
        image_prompt_payload: row.get("image_prompt_payload"),
        preview_asset_key: row.get("preview_asset_key"),
        failure_reason: row.get("failure_reason"),
        confirmed_at: row.get("confirmed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_static_page_render_output_row(
    row: &sqlx::postgres::PgRow,
) -> Result<StaticPageRenderOutput> {
    let status = row.get::<String, _>("status");
    Ok(StaticPageRenderOutput {
        id: StaticPageRenderOutputId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        draft_id: StaticPageDraftId(row.get::<Uuid, _>("draft_id")),
        assistant_run_id: AssistantRunId(row.get::<Uuid, _>("assistant_run_id")),
        image_job_id: row
            .get::<Option<Uuid>, _>("image_job_id")
            .map(StaticPageImageJobId),
        status: StaticPageRenderOutputStatus::from_str(&status)
            .ok_or_else(|| anyhow!("unknown static page render output status: {status}"))?,
        html: row.get("html"),
        asset_manifest: row.get("asset_manifest"),
        created_at: row.get("created_at"),
    })
}

fn map_chat_session_row(row: &sqlx::postgres::PgRow) -> Result<ChatSession> {
    Ok(ChatSession {
        id: ChatSessionId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        user_id: row.get::<Option<Uuid>, _>("user_id").map(UserId),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        title: row.get("title"),
        latest_memory_directory_id: row
            .get::<Option<Uuid>, _>("latest_memory_directory_id")
            .map(MemoryDirectoryId),
        latest_dataset_output_id: row
            .get::<Option<Uuid>, _>("latest_dataset_output_id")
            .map(DatasetOutputId),
        session_manifest: row.get("session_manifest"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_chat_message_row(row: &sqlx::postgres::PgRow) -> Result<ChatMessage> {
    let role = row.get::<String, _>("role");

    Ok(ChatMessage {
        id: ChatMessageId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        session_id: ChatSessionId(row.get::<Uuid, _>("session_id")),
        role: ChatMessageRole::from_str(&role)
            .ok_or_else(|| anyhow!("unknown chat message role: {role}"))?,
        turn_index: row.get("turn_index"),
        content: row.get("content"),
        message_manifest: row.get("message_manifest"),
        created_at: row.get("created_at"),
    })
}

fn map_llm_invocation_row(row: &sqlx::postgres::PgRow) -> Result<LlmInvocation> {
    let source_kind = row.get::<String, _>("source_kind");
    let mode = row.get::<String, _>("mode");
    let finish_reason = row.get::<Option<String>, _>("finish_reason");
    let usage = row
        .get::<Option<Value>, _>("usage")
        .map(parse_llm_token_usage)
        .transpose()?;
    let latency_ms = row
        .get::<Option<i64>, _>("latency_ms")
        .map(|value| u64::try_from(value).map_err(|error| anyhow!("invalid latency_ms: {error}")))
        .transpose()?;
    let tool_trace_count = row
        .get::<Option<i32>, _>("tool_trace_count")
        .map(|value| {
            usize::try_from(value).map_err(|error| anyhow!("invalid tool_trace_count: {error}"))
        })
        .transpose()?;

    Ok(LlmInvocation {
        id: LlmInvocationId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        source_kind: LlmInvocationSourceKind::from_str(&source_kind)
            .ok_or_else(|| anyhow!("unknown llm invocation source kind: {source_kind}"))?,
        dataset_output_id: row
            .get::<Option<Uuid>, _>("dataset_output_id")
            .map(DatasetOutputId),
        chat_message_id: row
            .get::<Option<Uuid>, _>("chat_message_id")
            .map(ChatMessageId),
        sequence_no: row.get("sequence_no"),
        mode: LlmInvocationMode::from_str(&mode)
            .ok_or_else(|| anyhow!("unknown llm invocation mode: {mode}"))?,
        provider: row.get("provider"),
        model: row.get("model"),
        request_id: row.get("request_id"),
        finish_reason: finish_reason
            .as_deref()
            .map(LlmInvocationFinishReason::from_str),
        latency_ms,
        usage,
        system_prompt_key: row.get("system_prompt_key"),
        system_prompt_version: row.get("system_prompt_version"),
        tool_trace_count,
        created_at: row.get("created_at"),
    })
}

fn map_tool_execution_row(row: &sqlx::postgres::PgRow) -> Result<ToolExecution> {
    let source_kind = row.get::<String, _>("source_kind");
    let status = row.get::<String, _>("status");

    Ok(ToolExecution {
        id: ToolExecutionId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        source_kind: ToolExecutionSourceKind::from_str(&source_kind)
            .ok_or_else(|| anyhow!("unknown tool execution source kind: {source_kind}"))?,
        dataset_output_id: row
            .get::<Option<Uuid>, _>("dataset_output_id")
            .map(DatasetOutputId),
        chat_message_id: row
            .get::<Option<Uuid>, _>("chat_message_id")
            .map(ChatMessageId),
        sequence_no: row.get("sequence_no"),
        call_id: row.get("call_id"),
        tool_name: row.get("tool_name"),
        tool_snapshot: row.get("tool_snapshot"),
        status: ToolExecutionStatus::from_str(&status)
            .ok_or_else(|| anyhow!("unknown tool execution status: {status}"))?,
        arguments: row.get("arguments"),
        result: row.get("result"),
        created_at: row.get("created_at"),
    })
}

fn map_workflow_execution_row(row: &sqlx::postgres::PgRow) -> Result<WorkflowExecution> {
    let kind = row.get::<String, _>("kind");
    let status = row.get::<String, _>("status");

    Ok(WorkflowExecution {
        id: WorkflowExecutionId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: row.get::<Option<Uuid>, _>("dataset_id").map(DatasetId),
        report_plan_id: row
            .get::<Option<Uuid>, _>("report_plan_id")
            .map(ReportPlanId),
        kind: WorkflowKind::from_str(&kind)
            .ok_or_else(|| anyhow!("unknown workflow kind: {kind}"))?,
        version: row.get("version"),
        stage: row.get("stage"),
        status: WorkflowStatus::from_str(&status)
            .ok_or_else(|| anyhow!("unknown workflow status: {status}"))?,
        attempt: row.get::<i32, _>("attempt") as u32,
        context: row.get("context"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_workflow_event_row(row: &sqlx::postgres::PgRow) -> Result<WorkflowEventRecord> {
    Ok(WorkflowEventRecord {
        id: WorkflowEventId(row.get::<Uuid, _>("id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        sequence_no: row.get("sequence_no"),
        event_name: row.get("event_name"),
        payload: row.get("payload"),
        created_at: row.get("created_at"),
    })
}

fn map_workflow_task_row(row: &sqlx::postgres::PgRow) -> Result<WorkflowTask> {
    let status = row.get::<String, _>("status");

    Ok(WorkflowTask {
        id: WorkflowTaskId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        queue: row.get("queue"),
        task_key: row.get("task_key"),
        payload: row.get("payload"),
        status: WorkflowTaskStatus::from_str(&status)
            .ok_or_else(|| anyhow!("unknown workflow task status: {status}"))?,
        attempt: row.get::<i32, _>("attempt") as u32,
        max_attempts: row.get::<i32, _>("max_attempts") as u32,
        available_at: row.get("available_at"),
        claimed_at: row.get("claimed_at"),
        finished_at: row.get("finished_at"),
        error: row.get("error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn insert_workflow_execution<'e, E>(executor: E, execution: &WorkflowExecution) -> Result<()>
where
    E: Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        insert into workflow_executions (
            id,
            tenant_id,
            dataset_id,
            report_plan_id,
            kind,
            version,
            stage,
            status,
            attempt,
            context,
            created_at,
            updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(execution.id.0)
    .bind(execution.tenant_id.0)
    .bind(execution.dataset_id.map(|value| value.0))
    .bind(execution.report_plan_id.map(|value| value.0))
    .bind(execution.kind.as_str())
    .bind(&execution.version)
    .bind(&execution.stage)
    .bind(execution.status.as_str())
    .bind(execution.attempt as i32)
    .bind(&execution.context)
    .bind(execution.created_at)
    .bind(execution.updated_at)
    .execute(executor)
    .await?;

    Ok(())
}

async fn insert_workflow_event<'e, E>(executor: E, event: &WorkflowEventRecord) -> Result<()>
where
    E: Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        insert into workflow_events (id, execution_id, sequence_no, event_name, payload, created_at)
        values ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(event.id.0)
    .bind(event.execution_id.0)
    .bind(event.sequence_no)
    .bind(&event.event_name)
    .bind(&event.payload)
    .bind(event.created_at)
    .execute(executor)
    .await?;

    Ok(())
}

async fn insert_workflow_task<'e, E>(
    executor: E,
    execution: &WorkflowExecution,
    task: &NewWorkflowTask,
    created_at: DateTime<Utc>,
) -> Result<WorkflowTask>
where
    E: Executor<'e, Database = sqlx::Postgres>,
{
    let persisted = WorkflowTask {
        id: WorkflowTaskId::new(),
        tenant_id: execution.tenant_id,
        execution_id: execution.id,
        queue: task.queue.clone(),
        task_key: task.task_key.clone(),
        payload: task.payload.clone(),
        status: WorkflowTaskStatus::Queued,
        attempt: 0,
        max_attempts: task.max_attempts,
        available_at: task.available_at,
        claimed_at: None,
        finished_at: None,
        error: None,
        created_at,
        updated_at: created_at,
    };

    sqlx::query(
        r#"
        insert into workflow_tasks (
            id,
            tenant_id,
            execution_id,
            queue,
            task_key,
            payload,
            status,
            attempt,
            max_attempts,
            available_at,
            claimed_at,
            finished_at,
            error,
            created_at,
            updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#,
    )
    .bind(persisted.id.0)
    .bind(persisted.tenant_id.0)
    .bind(persisted.execution_id.0)
    .bind(&persisted.queue)
    .bind(&persisted.task_key)
    .bind(&persisted.payload)
    .bind(persisted.status.as_str())
    .bind(persisted.attempt as i32)
    .bind(persisted.max_attempts as i32)
    .bind(persisted.available_at)
    .bind(persisted.claimed_at)
    .bind(persisted.finished_at)
    .bind(&persisted.error)
    .bind(persisted.created_at)
    .bind(persisted.updated_at)
    .execute(executor)
    .await?;

    Ok(persisted)
}

fn json_object_to_btree_map(value: Value) -> Result<BTreeMap<String, Value>> {
    match value {
        Value::Object(map) => Ok(map.into_iter().collect()),
        Value::Null => Ok(BTreeMap::new()),
        other => Err(anyhow!("expected JSON object, got {other}")),
    }
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn dataset_initial_metadata(metadata: &Value) -> Result<Value> {
    let mut merged = metadata.as_object().cloned().unwrap_or_else(Map::new);
    let visibility = merged
        .get("visibility")
        .and_then(Value::as_str)
        .and_then(DatasetVisibility::from_str)
        .unwrap_or(DatasetVisibility::Public);
    merged.insert(
        "visibility".to_string(),
        Value::String(visibility.as_str().to_string()),
    );
    if !merged.contains_key("default_secret_binding_ids") {
        merged.insert(
            "default_secret_binding_ids".to_string(),
            Value::Array(Vec::new()),
        );
    }
    secret_binding_ids_from_metadata_key(
        &Value::Object(merged.clone()),
        "default_secret_binding_ids",
    )?;
    Ok(Value::Object(merged))
}

fn dataset_visibility_from_metadata(metadata: &Value) -> DatasetVisibility {
    metadata
        .as_object()
        .and_then(|map| map.get("visibility"))
        .and_then(Value::as_str)
        .and_then(DatasetVisibility::from_str)
        .unwrap_or(DatasetVisibility::Public)
}

fn document_metadata_with_secret_binding_ids(secret_binding_ids: &[SecretBindingId]) -> Value {
    Value::Object(Map::from_iter([(
        "secret_binding_ids".to_string(),
        Value::Array(
            secret_binding_ids
                .iter()
                .map(|binding_id| Value::String(binding_id.to_string()))
                .collect(),
        ),
    )]))
}

fn document_initial_metadata(
    secret_binding_ids: &[SecretBindingId],
    metadata: &Value,
) -> Result<Value> {
    let mut merged = metadata.as_object().cloned().unwrap_or_else(Map::new);
    let secret_metadata = document_metadata_with_secret_binding_ids(secret_binding_ids);
    if let Some(secret_object) = secret_metadata.as_object() {
        for (key, value) in secret_object {
            merged.insert(key.clone(), value.clone());
        }
    }
    Ok(Value::Object(merged))
}

fn merge_dataset_metadata(
    current_metadata: &BTreeMap<String, Value>,
    metadata_updates: &Value,
) -> Result<Value> {
    let updates = metadata_updates
        .as_object()
        .ok_or_else(|| anyhow!("dataset metadata updates must be a JSON object"))?;
    let mut merged = Map::from_iter(current_metadata.clone());

    for (key, value) in updates {
        merged.insert(key.clone(), value.clone());
    }

    dataset_initial_metadata(&Value::Object(merged))
}

fn secret_binding_ids_from_metadata(metadata: &Value) -> Result<Vec<SecretBindingId>> {
    secret_binding_ids_from_metadata_key(metadata, "secret_binding_ids")
}

fn secret_binding_ids_from_metadata_key(
    metadata: &Value,
    key: &str,
) -> Result<Vec<SecretBindingId>> {
    let Some(entries) = metadata
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };

    entries
        .iter()
        .map(|entry| {
            let raw = entry
                .as_str()
                .ok_or_else(|| anyhow!("{key} entries must be strings"))?;
            Uuid::parse_str(raw)
                .map(SecretBindingId)
                .map_err(|error| anyhow!("invalid secret binding id {raw}: {error}"))
        })
        .collect()
}

fn retrieval_evidence_ids_to_uuid_array(ids: &[RetrievalEvidenceId]) -> Vec<Uuid> {
    ids.iter().map(|id| id.0).collect()
}

fn document_ids_to_uuid_array(ids: &[DocumentId]) -> Vec<Uuid> {
    ids.iter().map(|id| id.0).collect()
}

fn retrieval_lexical_query_terms(query: &str) -> Vec<String> {
    const CJK_NGRAM_MAX: usize = 6;

    fn is_cjk(value: char) -> bool {
        matches!(
            value as u32,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
        )
    }

    fn flush_ascii(terms: &mut BTreeSet<String>, ascii: &mut String) {
        if ascii.len() >= 2 {
            terms.insert(ascii.to_ascii_lowercase());
        }
        ascii.clear();
    }

    fn flush_cjk(terms: &mut BTreeSet<String>, cjk: &mut Vec<char>) {
        if cjk.is_empty() {
            return;
        }
        for value in cjk.iter() {
            terms.insert(value.to_string());
        }
        for ngram_size in 2..=CJK_NGRAM_MAX.min(cjk.len()) {
            for window in cjk.windows(ngram_size) {
                terms.insert(window.iter().collect::<String>());
            }
        }
        cjk.clear();
    }

    let mut terms = BTreeSet::new();
    let mut ascii = String::new();
    let mut cjk = Vec::new();

    for value in query.chars() {
        if value.is_ascii_alphanumeric() {
            flush_cjk(&mut terms, &mut cjk);
            ascii.push(value);
        } else if is_cjk(value) {
            flush_ascii(&mut terms, &mut ascii);
            cjk.push(value);
        } else {
            flush_ascii(&mut terms, &mut ascii);
            flush_cjk(&mut terms, &mut cjk);
        }
    }
    flush_ascii(&mut terms, &mut ascii);
    flush_cjk(&mut terms, &mut cjk);

    terms.into_iter().collect()
}

#[derive(Clone, Copy, Debug)]
enum LlmInvocationReplaceTarget {
    DatasetOutput(DatasetOutputId),
    ChatMessage(ChatMessageId),
    WorkflowExecution,
}

#[derive(Clone, Copy, Debug)]
enum ToolExecutionReplaceTarget {
    DatasetOutput(DatasetOutputId),
    ChatMessage(ChatMessageId),
    WorkflowExecution,
}

#[derive(Clone, Debug)]
struct ParsedManifestLlmInvocation {
    mode: LlmInvocationMode,
    provider: Option<String>,
    model: Option<String>,
    request_id: Option<String>,
    finish_reason: Option<LlmInvocationFinishReason>,
    latency_ms: Option<u64>,
    usage: Option<LlmTokenUsage>,
    system_prompt_key: Option<String>,
    system_prompt_version: Option<String>,
    tool_trace_count: Option<usize>,
}

#[derive(Clone, Debug)]
struct ParsedManifestToolCall {
    call_id: Option<String>,
    tool_name: String,
    tool_snapshot: Option<Value>,
    status: ToolExecutionStatus,
    arguments: Option<Value>,
    result: Option<Value>,
}

impl From<ParsedManifestLlmInvocation> for LlmInvocationRecordInput {
    fn from(value: ParsedManifestLlmInvocation) -> Self {
        Self {
            mode: value.mode,
            provider: value.provider,
            model: value.model,
            request_id: value.request_id,
            finish_reason: value.finish_reason,
            latency_ms: value.latency_ms,
            usage: value.usage,
            system_prompt_key: value.system_prompt_key,
            system_prompt_version: value.system_prompt_version,
            tool_trace_count: value.tool_trace_count,
        }
    }
}

impl From<ParsedManifestToolCall> for ToolExecutionRecordInput {
    fn from(value: ParsedManifestToolCall) -> Self {
        Self {
            call_id: value.call_id,
            tool_name: value.tool_name,
            tool_snapshot: value.tool_snapshot,
            status: value.status,
            arguments: value.arguments,
            result: value.result,
        }
    }
}

fn parse_llm_token_usage(value: Value) -> Result<LlmTokenUsage> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("manifest runtime usage must be an object"))?;
    let input_tokens = object
        .get("input_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("manifest runtime usage.input_tokens must be an integer"))?;
    let output_tokens = object
        .get("output_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("manifest runtime usage.output_tokens must be an integer"))?;
    let total_tokens = object
        .get("total_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("manifest runtime usage.total_tokens must be an integer"))?;

    Ok(LlmTokenUsage {
        input_tokens: usize::try_from(input_tokens)
            .map_err(|error| anyhow!("invalid input_tokens: {error}"))?,
        output_tokens: usize::try_from(output_tokens)
            .map_err(|error| anyhow!("invalid output_tokens: {error}"))?,
        total_tokens: usize::try_from(total_tokens)
            .map_err(|error| anyhow!("invalid total_tokens: {error}"))?,
    })
}

fn parse_manifest_llm_invocations(manifest: &Value) -> Result<Vec<LlmInvocationRecordInput>> {
    let Some(runtime) = manifest
        .as_object()
        .and_then(|object| object.get("runtime"))
    else {
        return Ok(Vec::new());
    };

    let object = runtime
        .as_object()
        .ok_or_else(|| anyhow!("manifest field runtime must be an object"))?;
    let mode_raw = object
        .get("mode")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest runtime must include mode"))?;
    let mode = LlmInvocationMode::from_str(mode_raw)
        .ok_or_else(|| anyhow!("unknown llm invocation mode: {mode_raw}"))?;
    let latency_ms = object
        .get("latency_ms")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| anyhow!("manifest runtime latency_ms must be an integer"))
        })
        .transpose()?;
    let usage = object
        .get("usage")
        .cloned()
        .map(parse_llm_token_usage)
        .transpose()?;
    let tool_trace_count = object
        .get("tool_trace_count")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| anyhow!("manifest runtime tool_trace_count must be an integer"))
                .and_then(|count| {
                    usize::try_from(count)
                        .map_err(|error| anyhow!("invalid tool_trace_count: {error}"))
                })
        })
        .transpose()?;

    Ok(vec![ParsedManifestLlmInvocation {
        mode,
        provider: object
            .get("provider")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: object
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        request_id: object
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        finish_reason: object
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(LlmInvocationFinishReason::from_str),
        latency_ms,
        usage,
        system_prompt_key: object
            .get("system_prompt_key")
            .and_then(Value::as_str)
            .map(str::to_string),
        system_prompt_version: object
            .get("system_prompt_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        tool_trace_count,
    }
    .into()])
}

fn parse_manifest_tool_calls(manifest: &Value) -> Result<Vec<ToolExecutionRecordInput>> {
    let Some(tool_trace) = manifest
        .as_object()
        .and_then(|object| object.get("tool_trace"))
    else {
        return Ok(Vec::new());
    };

    let entries = tool_trace
        .as_array()
        .ok_or_else(|| anyhow!("manifest field tool_trace must be an array"))?;
    entries
        .iter()
        .map(parse_manifest_tool_call)
        .map(|result| result.map(Into::into))
        .collect()
}

fn parse_manifest_tool_call(value: &Value) -> Result<ParsedManifestToolCall> {
    match value {
        Value::String(tool_name) => Ok(ParsedManifestToolCall {
            call_id: None,
            tool_name: tool_name.clone(),
            tool_snapshot: None,
            status: ToolExecutionStatus::Completed,
            arguments: None,
            result: None,
        }),
        Value::Object(object) => {
            let tool_name = object
                .get("tool_name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("manifest tool_trace entries must include tool_name"))?
                .to_string();
            let status_raw = object
                .get("status")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("manifest tool_trace entries must include status"))?;
            let status = ToolExecutionStatus::from_str(status_raw)
                .ok_or_else(|| anyhow!("unknown tool execution status: {status_raw}"))?;

            let tool_snapshot = match object.get("tool") {
                Some(Value::Object(_)) => object.get("tool").cloned(),
                Some(Value::Null) | None => None,
                Some(_) => {
                    return Err(anyhow!(
                        "manifest tool_trace entry tool field must be an object when present"
                    ))
                }
            };

            Ok(ParsedManifestToolCall {
                call_id: object
                    .get("call_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                tool_name,
                tool_snapshot,
                status,
                arguments: object.get("arguments").cloned(),
                result: object.get("result").cloned(),
            })
        }
        _ => Err(anyhow!(
            "manifest tool_trace entries must be strings or objects"
        )),
    }
}

fn merge_document_metadata(
    current_metadata: &BTreeMap<String, Value>,
    metadata_updates: &Value,
) -> Result<Value> {
    let updates = metadata_updates
        .as_object()
        .ok_or_else(|| anyhow!("document metadata updates must be a JSON object"))?;
    let mut merged = Map::from_iter(current_metadata.clone());

    for (key, value) in updates {
        merged.insert(key.clone(), value.clone());
    }

    Ok(Value::Object(merged))
}

fn merge_chunk_metadata(
    current_metadata: &BTreeMap<String, Value>,
    metadata_updates: &Value,
) -> Result<Value> {
    let updates = metadata_updates
        .as_object()
        .ok_or_else(|| anyhow!("document chunk metadata updates must be a JSON object"))?;
    let mut merged = Map::from_iter(current_metadata.clone());

    for (key, value) in updates {
        merged.insert(key.clone(), value.clone());
    }

    Ok(Value::Object(merged))
}

pub fn external_integration_config_redacted_summary(config: &Value) -> Value {
    redact_external_config_value(None, config)
}

fn redact_external_config_value(parent_key: Option<&str>, value: &Value) -> Value {
    if parent_key
        .map(external_integration_config_key_is_sensitive)
        .unwrap_or(false)
    {
        return Value::String("[redacted]".to_string());
    }

    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        redact_external_config_value(Some(key.as_str()), value),
                    )
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| redact_external_config_value(parent_key, value))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn external_integration_config_key_is_sensitive(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "secret",
        "token",
        "password",
        "private_key",
        "api_key",
        "access_key",
        "refresh_key",
        "signing_key",
        "encrypt_key",
        "credential",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map};

    #[test]
    fn configured_database_max_connections_prefers_service_then_global() {
        assert_eq!(parse_configured_database_max_connections(None, None), 10);
        assert_eq!(
            parse_configured_database_max_connections(Some("20"), Some("30")),
            20
        );
        assert_eq!(
            parse_configured_database_max_connections(Some("0"), Some("30")),
            30
        );
        assert_eq!(
            parse_configured_database_max_connections(Some("bad"), Some("8")),
            8
        );
        assert_eq!(
            parse_configured_database_max_connections(Some("150"), None),
            100
        );
    }

    #[test]
    fn initial_schema_mentions_primary_tables() {
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists workflow_executions"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists report_plans"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists document_chunks"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists document_facts"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists document_fact_sources"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists dataset_fact_snapshots"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists report_plan_ast_versions"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists report_render_outputs"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists memory_directories"));
        assert!(INITIAL_SCHEMA.sql.contains("version_no integer not null"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists retrieval_evidences"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists dataset_outputs"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists assistant_runs"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists assistant_run_events"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists chat_sessions"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists chat_messages"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists llm_invocations"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists tool_executions"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists secret_bindings"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists workflow_tasks"));
    }

    #[test]
    fn migrations_include_retrieval_lexical_index_schema() {
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.version == "0014"
                && migration.description == "retrieval evidence lexical search index"));
        assert!(RETRIEVAL_LEXICAL_INDEX_SCHEMA
            .sql
            .contains("retrieval_evidences_search_terms_gin_idx"));
        assert!(RETRIEVAL_LEXICAL_INDEX_SCHEMA
            .sql
            .contains("indexed_content_hash"));
    }

    #[test]
    fn migrations_include_enterprise_asset_library_schema() {
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.version == "0015"
                && migration.description == "enterprise asset libraries"));
        for table in [
            "enterprise_asset_libraries",
            "asset_library_dataset_memberships",
            "asset_collections",
            "asset_items",
            "dataset_asset_memberships",
            "asset_profiles",
        ] {
            assert!(TABLES.contains(&table));
            assert!(ASSET_LIBRARIES_SCHEMA
                .sql
                .contains(&format!("create table if not exists {table}")));
        }
        assert!(ASSET_LIBRARIES_SCHEMA
            .sql
            .contains("primary key (tenant_id, asset_library_id, dataset_id)"));
        assert!(ASSET_LIBRARIES_SCHEMA
            .sql
            .contains("asset_library_dataset_memberships_priority_idx"));
        assert!(ASSET_LIBRARIES_SCHEMA
            .sql
            .contains("unique (tenant_id, asset_library_id, external_id)"));
        assert!(ASSET_LIBRARIES_SCHEMA
            .sql
            .contains("primary key (tenant_id, dataset_id, asset_id)"));
        assert!(ASSET_LIBRARIES_SCHEMA
            .sql
            .contains("unique (tenant_id, asset_id, profile_kind, profile_version)"));
        assert!(ASSET_LIBRARIES_SCHEMA
            .sql
            .contains("asset_items_source_idx"));
        assert!(ASSET_LIBRARIES_SCHEMA
            .sql
            .contains("asset_items_source_unique_idx"));
        assert!(!ASSET_LIBRARIES_SCHEMA.sql.contains("asset_parse_runs"));
        assert!(ASSET_LIBRARIES_SCHEMA
            .sql
            .contains("asset_profiles_attributes_gin_idx"));
    }

    #[test]
    fn migrations_include_asset_parse_runs_schema() {
        let parse_runs_position = MIGRATIONS
            .iter()
            .position(|migration| migration.version == "0017")
            .expect("asset parse runs migration");
        let asset_evidence_position = MIGRATIONS
            .iter()
            .position(|migration| migration.version == "0018")
            .expect("asset retrieval evidence migration");
        assert!(parse_runs_position < asset_evidence_position);
        assert!(TABLES.contains(&"asset_parse_runs"));
        assert!(ASSET_PARSE_RUNS_SCHEMA
            .sql
            .contains("create table if not exists asset_parse_runs"));
        assert!(ASSET_PARSE_RUNS_SCHEMA
            .sql
            .contains("unique (tenant_id, asset_id, parser_name, parser_version)"));
        assert!(ASSET_PARSE_RUNS_SCHEMA
            .sql
            .contains("asset_parse_runs_status_idx"));
    }

    #[test]
    fn migrations_include_asset_retrieval_evidence_schema() {
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.version == "0018"));
        assert!(TABLES.contains(&"asset_retrieval_evidences"));
        assert!(ASSET_RETRIEVAL_EVIDENCES_SCHEMA
            .sql
            .contains("create table if not exists asset_retrieval_evidences"));
        for required_column in [
            "asset_id uuid not null references asset_items",
            "asset_profile_id uuid not null references asset_profiles",
            "profile_kind text not null",
            "profile_version text not null",
            "materialized_text text not null",
            "safe_metadata jsonb not null",
            "content_hash text not null",
        ] {
            assert!(ASSET_RETRIEVAL_EVIDENCES_SCHEMA
                .sql
                .contains(required_column));
        }
        assert!(ASSET_RETRIEVAL_EVIDENCES_SCHEMA.sql.contains(
            "unique (tenant_id, dataset_id, asset_id, profile_kind, profile_version, content_hash)"
        ));
        assert!(!ASSET_RETRIEVAL_EVIDENCES_SCHEMA.sql.contains("document_id"));
        assert!(!ASSET_RETRIEVAL_EVIDENCES_SCHEMA
            .sql
            .contains("document_chunk_id"));
    }

    #[test]
    fn migrations_include_dataset_semantic_understanding_schema() {
        let semantic_snapshot_position = MIGRATIONS
            .iter()
            .position(|migration| migration.version == "0019")
            .expect("dataset semantic understanding migration");
        let cross_graph_position = MIGRATIONS
            .iter()
            .position(|migration| migration.version == "0020")
            .expect("dataset semantic cross-graph migration");
        assert!(semantic_snapshot_position < cross_graph_position);
        for table in ["dataset_semantic_snapshots", "semantic_dictionary_entries"] {
            assert!(TABLES.contains(&table));
            assert!(DATASET_SEMANTIC_UNDERSTANDING_SCHEMA
                .sql
                .contains(&format!("create table if not exists {table}")));
        }
        assert!(DATASET_SEMANTIC_UNDERSTANDING_SCHEMA
            .sql
            .contains("unique (tenant_id, dataset_id, generation_version, source_fingerprint)"));
        assert!(DATASET_SEMANTIC_UNDERSTANDING_SCHEMA
            .sql
            .contains("where status = 'ready'"));
        assert!(DATASET_SEMANTIC_UNDERSTANDING_SCHEMA
            .sql
            .contains("check (status in ('building', 'ready', 'failed', 'superseded'))"));
        assert!(!DATASET_SEMANTIC_UNDERSTANDING_SCHEMA
            .sql
            .contains("manifest_gin"));
    }

    #[test]
    fn dataset_semantic_repository_queries_are_tenant_scoped_and_ready_safe() {
        assert!(DATASET_SEMANTIC_LATEST_READY_SQL.contains("tenant_id = $1"));
        assert!(DATASET_SEMANTIC_LATEST_READY_SQL.contains("dataset_id = $2"));
        assert!(DATASET_SEMANTIC_LATEST_READY_SQL.contains("status = 'ready'"));
        assert!(DATASET_SEMANTIC_BEGIN_BUILD_SQL.contains("on conflict"));
        assert!(DATASET_SEMANTIC_BEGIN_BUILD_SQL.contains("status in ('failed', 'superseded')"));
        assert!(DATASET_SEMANTIC_MARK_READY_SQL.contains("status = 'building'"));
        assert!(DATASET_SEMANTIC_MARK_FAILED_SQL.contains("status = 'building'"));
        assert!(SEMANTIC_DICTIONARY_RESOLVE_SQL.contains("tenant_id = $1"));
        assert!(DATASET_SEMANTIC_ACTIVE_MEMBERSHIPS_SQL.contains("tenant_id = $1"));
        assert!(DATASET_SEMANTIC_ACTIVE_MEMBERSHIPS_SQL.contains("dataset_id = $2"));
        assert!(DATASET_SEMANTIC_ACTIVE_MEMBERSHIPS_SQL.contains("created_at <= $3"));
        assert!(DATASET_SEMANTIC_ACTIVE_MEMBERSHIPS_SQL.contains("expires_at > $3"));
        assert!(DATASET_SEMANTIC_READY_BY_ID_SQL.contains("tenant_id = $1"));
        assert!(DATASET_SEMANTIC_READY_BY_ID_SQL.contains("dataset_id = $2"));
        assert!(DATASET_SEMANTIC_READY_BY_ID_SQL.contains("id = $3"));
        assert!(DATASET_SEMANTIC_READY_BY_ID_SQL.contains("status = 'ready'"));
        assert!(DATASET_SEMANTIC_LATEST_READY_BY_TENANT_SQL.contains("tenant_id = $1"));
        assert!(DATASET_SEMANTIC_LATEST_READY_BY_TENANT_SQL.contains("distinct on (dataset_id)"));
    }

    #[test]
    fn dataset_semantic_link_pair_canonicalization_orders_uuid_and_keeps_snapshot_alignment() {
        let lower_dataset =
            DatasetId(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
        let higher_dataset =
            DatasetId(Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap());
        let lower_snapshot = Uuid::parse_str("10000000-0000-0000-0000-000000000001").unwrap();
        let higher_snapshot = Uuid::parse_str("20000000-0000-0000-0000-000000000002").unwrap();

        let canonical = canonical_dataset_semantic_link_pair(
            higher_dataset,
            lower_dataset,
            higher_snapshot,
            lower_snapshot,
        )
        .expect("distinct dataset pair");

        assert_eq!(canonical.0, lower_dataset);
        assert_eq!(canonical.1, higher_dataset);
        assert_eq!(canonical.2, lower_snapshot);
        assert_eq!(canonical.3, higher_snapshot);
        assert!(canonical_dataset_semantic_link_pair(
            lower_dataset,
            lower_dataset,
            lower_snapshot,
            higher_snapshot,
        )
        .is_err());
    }

    #[test]
    fn dataset_semantic_link_manifest_and_counts_are_validated_before_sql() {
        assert!(validate_dataset_semantic_link_ready_payload(&json!({"version": 1}), 3, 4).is_ok());
        assert!(validate_dataset_semantic_link_ready_payload(&json!([]), 3, 4).is_err());
        assert!(validate_dataset_semantic_link_ready_payload(&json!({}), -1, 4).is_err());
        assert!(validate_dataset_semantic_link_ready_payload(&json!({}), 3, -1).is_err());
    }

    #[test]
    fn dataset_semantic_link_terminal_run_state_fails_any_orphan_build() {
        assert!(dataset_semantic_link_run_status_requires_failed_build(
            "dead_letter"
        ));
        for non_terminal_status in ["pending", "running", "retry_wait", "succeeded"] {
            assert!(!dataset_semantic_link_run_status_requires_failed_build(
                non_terminal_status
            ));
        }
    }

    #[test]
    fn dataset_semantic_link_migration_has_pair_and_state_machine_constraints() {
        assert_eq!(
            MIGRATIONS.last().map(|migration| migration.version),
            Some("0020")
        );
        for table in [
            "dataset_semantic_link_snapshots",
            "dataset_semantic_link_runs",
        ] {
            assert!(TABLES.contains(&table));
            assert!(DATASET_SEMANTIC_CROSS_GRAPH_SCHEMA
                .sql
                .contains(&format!("create table if not exists {table}")));
        }
        let sql = DATASET_SEMANTIC_CROSS_GRAPH_SCHEMA.sql;
        for required in [
            "dataset_semantic_snapshots_tenant_dataset_id_uidx",
            "on dataset_semantic_snapshots (tenant_id, dataset_id, id)",
            "check (left_dataset_id < right_dataset_id)",
            "unique (tenant_id, left_snapshot_id, right_snapshot_id, generation_version)",
            "check (status in ('building', 'ready', 'failed', 'superseded'))",
            "check (jsonb_typeof(manifest) = 'object')",
            "check (node_count >= 0)",
            "check (edge_count >= 0)",
            "check (status in ('pending', 'running', 'succeeded', 'retry_wait', 'dead_letter'))",
            "check (attempt_count >= 0)",
            "check (max_attempts > 0)",
            "available_at timestamptz not null",
        ] {
            assert!(
                sql.contains(required),
                "missing migration constraint: {required}"
            );
        }
        assert_eq!(
            sql.matches("foreign key (tenant_id, left_dataset_id, left_snapshot_id)")
                .count(),
            2
        );
        assert_eq!(
            sql.matches("foreign key (tenant_id, right_dataset_id, right_snapshot_id)")
                .count(),
            2
        );
        assert_eq!(
            sql.matches("references dataset_semantic_snapshots (tenant_id, dataset_id, id)")
                .count(),
            4
        );
        assert!(
            !sql.contains("snapshot_id uuid not null references dataset_semantic_snapshots (id)")
        );
    }

    #[test]
    fn dataset_semantic_link_repository_sql_is_tenant_scoped_and_transition_safe() {
        for (name, sql) in [
            ("latest ready", DATASET_SEMANTIC_LINK_LATEST_READY_SQL),
            ("latest attempt", DATASET_SEMANTIC_LINK_LATEST_ATTEMPT_SQL),
            ("exact inputs", DATASET_SEMANTIC_LINK_BY_INPUTS_SQL),
            ("mark ready", DATASET_SEMANTIC_LINK_MARK_READY_SQL),
            ("mark failed", DATASET_SEMANTIC_LINK_MARK_FAILED_SQL),
            ("claim run", DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_SQL),
            (
                "allowlisted claim run",
                DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_FOR_DATASETS_SQL,
            ),
            ("defer run", DATASET_SEMANTIC_LINK_DEFER_RUN_SQL),
            (
                "recover stale run",
                DATASET_SEMANTIC_LINK_RECOVER_STALE_RUNS_SQL,
            ),
            (
                "fail stale build",
                DATASET_SEMANTIC_LINK_FAIL_BUILD_FOR_RUN_SQL,
            ),
            (
                "stale dataset links",
                DATASET_SEMANTIC_LINK_MARK_DATASET_READY_STALE_SQL,
            ),
            (
                "revive dead letter",
                DATASET_SEMANTIC_LINK_REVIVE_DEAD_LETTER_RUN_SQL,
            ),
            ("retry run", DATASET_SEMANTIC_LINK_RETRY_OR_DEAD_LETTER_SQL),
            ("succeed run", DATASET_SEMANTIC_LINK_MARK_RUN_SUCCEEDED_SQL),
        ] {
            assert!(
                sql.contains("tenant_id = $1"),
                "{name} must be tenant scoped"
            );
        }
        for sql in [
            DATASET_SEMANTIC_LINK_BEGIN_BUILD_SQL,
            DATASET_SEMANTIC_LINK_CREATE_OR_GET_RUN_SQL,
        ] {
            assert!(sql.contains("tenant_id"));
            assert!(sql.contains("$1"));
            assert!(sql.contains("on conflict"));
        }
        assert!(DATASET_SEMANTIC_LINK_MARK_READY_SQL.contains("status = 'building'"));
        assert!(DATASET_SEMANTIC_LINK_BY_INPUTS_SQL.contains("left_snapshot_id = $4"));
        assert!(DATASET_SEMANTIC_LINK_BY_INPUTS_SQL.contains("right_snapshot_id = $5"));
        assert!(DATASET_SEMANTIC_LINK_MARK_FAILED_SQL.contains("status = 'building'"));
        assert!(DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_SQL.contains("for update skip locked"));
        assert!(DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_SQL.contains("attempt_count + 1"));
        assert!(DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_FOR_DATASETS_SQL
            .contains("left_dataset_id = any($3::uuid[])"));
        assert!(DATASET_SEMANTIC_LINK_CLAIM_READY_RUN_FOR_DATASETS_SQL
            .contains("right_dataset_id = any($3::uuid[])"));
        assert!(DATASET_SEMANTIC_LINK_DEFER_RUN_SQL
            .contains("attempt_count = greatest(attempt_count - 1, 0)"));
        assert!(DATASET_SEMANTIC_LINK_RECOVER_STALE_RUNS_SQL
            .contains("failure_code = 'worker_lease_expired'"));
        assert!(DATASET_SEMANTIC_LINK_RECOVER_STALE_RUNS_SQL
            .contains("left_dataset_id = any($4::uuid[])"));
        assert!(DATASET_SEMANTIC_LINK_RECOVER_STALE_RUNS_SQL
            .contains("right_dataset_id = any($4::uuid[])"));
        assert!(DATASET_SEMANTIC_LINK_FAIL_BUILD_FOR_RUN_SQL.contains("failure_code = $6"));
        assert!(DATASET_SEMANTIC_LINK_FAIL_BUILD_FOR_RUN_SQL.contains("updated_at = $7"));
        assert!(DATASET_SEMANTIC_LINK_FAIL_BUILD_FOR_RUN_SQL.contains("status = 'building'"));
        assert!(DATASET_SEMANTIC_LINK_MARK_PAIR_READY_STALE_SQL.contains("jsonb_set"));
        assert!(DATASET_SEMANTIC_LINK_MARK_DATASET_READY_STALE_SQL.contains("jsonb_set"));
        assert!(DATASET_SEMANTIC_LINK_REVIVE_DEAD_LETTER_RUN_SQL.contains("status = 'dead_letter'"));
        assert!(DATASET_SEMANTIC_LINK_RETRY_OR_DEAD_LETTER_SQL
            .contains("attempt_count >= max_attempts"));
        assert!(DATASET_SEMANTIC_LINK_RETRY_OR_DEAD_LETTER_SQL.contains("'dead_letter'"));
        assert!(DATASET_SEMANTIC_LINK_MARK_RUN_SUCCEEDED_SQL.contains("status = 'running'"));
    }

    #[test]
    fn dataset_semantic_dictionary_normalizes_empty_scope_without_nulls() {
        assert_eq!(normalized_dictionary_scope_key(""), "*");
        assert_eq!(normalized_dictionary_scope_key("  "), "*");
        assert_eq!(normalized_dictionary_scope_key("erp"), "erp");
        assert!(DATASET_SEMANTIC_UNDERSTANDING_SCHEMA
            .sql
            .contains("source_system_key text not null default '*'"));
        assert!(DATASET_SEMANTIC_UNDERSTANDING_SCHEMA
            .sql
            .contains("source_object_key text not null default '*'"));
    }

    #[test]
    fn migrations_include_v3_client_artifact_schema() {
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.version == "0016"
                && migration.description == "v3 client config packages and uploaded artifacts"));
        for table in [
            "v3_client_config_packages",
            "v3_client_artifacts",
            "v3_client_artifact_files",
        ] {
            assert!(TABLES.contains(&table));
            assert!(V3_CLIENT_ARTIFACTS_SCHEMA
                .sql
                .contains(&format!("create table if not exists {table}")));
        }
        assert!(V3_CLIENT_ARTIFACTS_SCHEMA
            .sql
            .contains("v3_client_artifacts_dataset_ids"));
        assert!(V3_CLIENT_ARTIFACTS_SCHEMA
            .sql
            .contains("unique (artifact_id, file_index)"));
        assert!(V3_CLIENT_ARTIFACTS_SCHEMA
            .sql
            .contains("storage_kind text not null default 'database'"));
        assert!(V3_CLIENT_ARTIFACTS_SCHEMA
            .sql
            .contains("v3_client_artifact_files_storage_kind_chk"));
        assert!(V3_CLIENT_ARTIFACTS_SCHEMA
            .sql
            .contains("v3_client_artifact_files_storage_payload_chk"));
    }

    #[test]
    fn image_document_asset_profile_extracts_vlm_semantics() {
        let document = test_profile_document(
            "充值记录.png",
            "image/png",
            json!({
                "vlm": {
                    "payload": {
                        "summary": "充值记录截图，包含订单号、支付方式和状态。",
                        "visualSummary": "后台充值记录表格截图。",
                        "documentKind": "order_screenshot",
                        "layoutType": "table",
                        "topicTags": ["充值记录", "订单号", "支付宝"],
                        "riskLevel": "low",
                        "chartOrTableDetected": true,
                        "tableLikeSignals": ["多列表格", "状态标签"],
                        "transcribedText": "订单号 A1778730534 支付宝 成功",
                        "entities": [{
                            "name": "支付宝",
                            "type": "payment_method"
                        }],
                        "fieldCandidates": [{
                            "key": "订单号",
                            "value": "A1778730534",
                            "source": "vlm",
                            "evidenceText": "订单号 A1778730534"
                        }]
                    }
                }
            }),
        );

        let attributes = document_asset_profile_attributes(&document, &[]);

        assert_eq!(
            attributes["summary"],
            json!("充值记录截图，包含订单号、支付方式和状态。")
        );
        assert_eq!(
            attributes["visual_summary"],
            json!("后台充值记录表格截图。")
        );
        assert_eq!(attributes["document_kind"], json!("order_screenshot"));
        assert_eq!(attributes["layout_type"], json!("table"));
        assert_eq!(attributes["chart_or_table_detected"], json!(true));
        assert_eq!(
            attributes["ocr_text"],
            json!("订单号 A1778730534 支付宝 成功")
        );
        assert_eq!(attributes["tags"], json!(["充值记录", "订单号", "支付宝"]));
        assert_eq!(attributes["entities"][0]["name"], json!("支付宝"));
        assert_eq!(attributes["field_candidates"][0]["key"], json!("订单号"));
        assert!(attributes["noun_terms"]
            .as_array()
            .expect("noun terms should be array")
            .iter()
            .any(|term| term == "支付宝"));
    }

    #[test]
    fn video_document_asset_profile_extracts_media_signals() {
        let document = test_profile_document("门店巡检.mp4", "video/mp4", json!({}));
        let chunk = test_profile_chunk(
            "Media file: 门店巡检.mp4",
            json!({
                "media": {
                    "kind": "video",
                    "parse_status": "enriched_partial",
                    "transcript_segments": [
                        {"start_seconds": 1.0, "end_seconds": 4.0, "text": "这里是夏季女装陈列区。", "source": "asr"},
                        {"start_seconds": 5.0, "end_seconds": 7.0, "text": "导购正在介绍促销活动。", "source": "asr"}
                    ],
                    "scenes": [{
                        "representative_seconds": 3.0,
                        "summary": "镜头扫过女装陈列货架",
                        "source": "scene-detector"
                    }],
                    "keyframe_ocr_snippets": [{
                        "timestamp_seconds": 3.5,
                        "text": "夏季女装 满减活动",
                        "source": "keyframe-ocr"
                    }],
                    "provider_evidence": [{
                        "capability": "keyframe_image_vlm",
                        "status": "supported",
                        "detail": "可复用图片 VLM"
                    }]
                }
            }),
        );

        let attributes = document_asset_profile_attributes(&document, &[chunk]);

        assert_eq!(attributes["media_kind"], json!("video"));
        assert_eq!(attributes["media_parse_status"], json!("enriched_partial"));
        assert_eq!(attributes["transcript_segment_count"], json!(2));
        assert_eq!(attributes["scene_count"], json!(1));
        assert_eq!(attributes["keyframe_ocr_snippet_count"], json!(1));
        assert!(attributes["transcript_summary"]
            .as_str()
            .unwrap_or_default()
            .contains("夏季女装陈列区"));
        assert_eq!(attributes["ocr_text"], json!("夏季女装 满减活动"));
        assert_eq!(
            attributes["scene_summaries"][0]["summary"],
            json!("镜头扫过女装陈列货架")
        );
        assert!(attributes["noun_terms"]
            .as_array()
            .expect("noun terms should be array")
            .iter()
            .any(|term| term == "镜头扫过女装陈列货架"));
    }

    #[test]
    fn presentation_document_asset_profile_builds_outline_from_chunks() {
        let document = test_profile_document(
            "经营复盘.pptx",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            json!({}),
        );
        let chunks = vec![
            test_profile_chunk(
                "销售趋势\n一月到三月销售持续改善",
                json!({"section_title_hints": ["销售趋势"]}),
            ),
            test_profile_chunk(
                "库存风险\n部分门店库存周转偏慢",
                json!({"section_title_hints": ["库存风险"]}),
            ),
        ];

        let attributes = document_asset_profile_attributes(&document, &chunks);

        assert_eq!(attributes["outline"], json!(["销售趋势", "库存风险"]));
        assert_eq!(attributes["slide_count_estimate"], json!(2));
        assert_eq!(
            attributes["slide_text_samples"].as_array().unwrap().len(),
            2
        );
        assert!(attributes["noun_terms"]
            .as_array()
            .expect("noun terms should be array")
            .iter()
            .any(|term| term == "销售趋势"));
    }

    fn test_profile_document(title: &str, content_type: &str, metadata: Value) -> Document {
        let now = Utc::now();
        Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: title.to_string(),
            object_key: format!("test/{title}"),
            content_type: content_type.to_string(),
            lifecycle: DocumentLifecycle::Extracted,
            secret_binding_ids: Vec::new(),
            metadata: value_object_btree(metadata),
            created_at: now,
            updated_at: now,
        }
    }

    fn test_profile_chunk(content: &str, metadata: Value) -> DocumentChunk {
        let now = Utc::now();
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunk_index: 0,
            content: content.to_string(),
            token_count: content.split_whitespace().count() as i32,
            state: DocumentChunkState::Extracted,
            metadata: value_object_btree(metadata),
            created_at: now,
            updated_at: now,
        }
    }

    fn value_object_btree(value: Value) -> BTreeMap<String, Value> {
        value
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    #[test]
    fn retrieval_evidence_upsert_populates_lexical_columns() {
        assert!(RETRIEVAL_EVIDENCE_UPSERT_SQL.contains("search_terms"));
        assert!(RETRIEVAL_EVIDENCE_UPSERT_SQL.contains("search_tsv"));
        assert!(RETRIEVAL_EVIDENCE_UPSERT_SQL.contains("indexed_content_hash"));
        assert!(RETRIEVAL_EVIDENCE_UPSERT_SQL.contains("'{lexical,search_text}'"));
        assert!(RETRIEVAL_EVIDENCE_UPSERT_SQL.contains("'{lexical,search_terms}'"));
    }

    #[test]
    fn asset_retrieval_evidence_insert_is_membership_and_terminal_profile_guarded() {
        assert!(ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL.contains("join dataset_asset_memberships"));
        assert!(ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL.contains("membership.expires_at"));
        assert!(ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL.contains("from asset_parse_runs"));
        assert!(ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL.contains("'completed', 'partial'"));
        assert!(ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL.contains("on conflict"));
        assert!(ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL.contains("do nothing"));
        assert!(!ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL.contains("document_id"));
        assert!(!ASSET_RETRIEVAL_EVIDENCE_INSERT_SQL.contains("document_chunk_id"));
    }

    #[test]
    fn retrieval_lexical_query_terms_include_cjk_ngrams_and_ascii() {
        let terms = retrieval_lexical_query_terms("订单AI延期 风险 up 最大");

        for expected in ["订", "订单", "ai", "延", "延期", "风险", "up", "最大"] {
            assert!(terms.contains(&expected.to_string()), "missing {expected}");
        }
        assert!(terms.contains(&"订单ai".to_string()) == false);
    }

    #[test]
    fn auth_migrations_are_registered_in_order() {
        assert_eq!(
            MIGRATIONS
                .iter()
                .map(|migration| migration.version)
                .collect::<Vec<_>>(),
            vec![
                "0001", "0002", "0004", "0005", "0006", "0007", "0008", "0009", "0010", "0011",
                "0012", "0013", "0014", "0015", "0016", "0017", "0018", "0019", "0020"
            ]
        );
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "email account authentication"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "account artifact hardening"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "memory directory scope hardening"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "safe html artifact records"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "external bot and third-party integrations"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "video ppt published version history"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "dataset document memberships"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "model gateway profiles"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "document fact index"));
        assert!(MIGRATIONS
            .iter()
            .any(|migration| migration.description == "document canonical enrichment"));
        assert!(TABLES.contains(&"published_video_ppt_packages"));
        assert!(TABLES.contains(&"published_video_ppt_versions"));
        assert!(VIDEO_PPT_PUBLISHED_VERSIONS_SCHEMA
            .sql
            .contains("create table if not exists published_video_ppt_packages"));
        assert!(VIDEO_PPT_PUBLISHED_VERSIONS_SCHEMA
            .sql
            .contains("create table if not exists published_video_ppt_versions"));
        assert!(VIDEO_PPT_PUBLISHED_VERSIONS_SCHEMA
            .sql
            .contains("unique (package_id, version_fingerprint)"));
    }

    #[test]
    fn document_fact_index_schema_mentions_tables_and_indexes() {
        assert!(TABLES.contains(&"document_facts"));
        assert!(TABLES.contains(&"document_fact_sources"));
        assert!(TABLES.contains(&"dataset_fact_snapshots"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists document_facts"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists document_fact_sources"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists dataset_fact_snapshots"));
        assert!(DOCUMENT_FACT_INDEX_SCHEMA
            .sql
            .contains("document_facts_dataset_type_name_idx"));
        assert!(DOCUMENT_FACT_INDEX_SCHEMA
            .sql
            .contains("document_facts_document_type_idx"));
        assert!(DOCUMENT_FACT_INDEX_SCHEMA
            .sql
            .contains("dataset_fact_snapshots_manifest_gin_idx"));
        assert!(DOCUMENT_FACT_INDEX_SCHEMA
            .sql
            .contains("primary key (tenant_id, dataset_id, snapshot_kind, snapshot_key)"));
    }

    #[test]
    fn document_canonical_enrichment_schema_mentions_tables_fields_and_indexes() {
        assert!(TABLES.contains(&"document_content_fingerprints"));
        assert!(TABLES.contains(&"document_enrichment_runs"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("add column if not exists content_sha256 text"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("add column if not exists content_size_bytes bigint"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("add column if not exists canonical_document_id uuid references documents"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("add column if not exists dedup_state text not null default 'unknown'"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("add column if not exists deduped_at timestamptz"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("create table if not exists document_content_fingerprints"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("primary key (tenant_id, content_sha256)"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("create table if not exists document_enrichment_runs"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("enrichment_kind text not null"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("input_fingerprint text not null"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("document_enrichment_runs_idempotency_idx"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("(tenant_id, document_id, enrichment_kind, input_fingerprint)"));
        assert!(DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA
            .sql
            .contains("document_enrichment_runs_status_priority_idx"));
    }

    #[test]
    fn dataset_document_membership_schema_mentions_table_and_indexes() {
        assert!(TABLES.contains(&"dataset_document_memberships"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists dataset_document_memberships"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("primary key (tenant_id, dataset_id, document_id)"));
        assert!(DATASET_DOCUMENT_MEMBERSHIPS_SCHEMA
            .sql
            .contains("create table if not exists dataset_document_memberships"));
        assert!(DATASET_DOCUMENT_MEMBERSHIPS_SCHEMA
            .sql
            .contains("dataset_document_memberships_document_idx"));
        assert!(DATASET_DOCUMENT_MEMBERSHIPS_SCHEMA
            .sql
            .contains("dataset_document_memberships_expiry_idx"));
    }

    #[test]
    fn dataset_membership_semantic_scope_unifies_direct_and_membership_documents() {
        assert!(DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL.contains("from documents d"));
        assert!(DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL.contains("from dataset_document_memberships m"));
        assert!(DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL.contains("union all"));
        assert!(DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL.contains("distinct on (scoped.id)"));
        assert_eq!(
            DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL
                .matches("tenant_id = $1")
                .count(),
            2
        );
        assert_eq!(
            DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL
                .matches("dataset_id = $2")
                .count(),
            2
        );
        assert!(DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL.contains("created_at <= $3"));
        assert!(DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL.contains("expires_at > $3"));
        assert!(DATASET_SEMANTIC_DOCUMENT_SCOPE_SQL.contains("limit $4"));
    }

    #[test]
    fn model_gateway_profiles_migration_is_registered() {
        let versions = MIGRATIONS
            .iter()
            .map(|migration| migration.version)
            .collect::<Vec<_>>();
        let dataset_membership_index = versions
            .iter()
            .position(|version| *version == "0010")
            .expect("dataset document memberships migration registered");
        let model_gateway_index = versions
            .iter()
            .position(|version| *version == "0011")
            .expect("model gateway profiles migration registered");

        assert!(
            model_gateway_index > dataset_membership_index,
            "model gateway profiles migration must run after dataset document memberships"
        );
    }

    #[test]
    fn model_gateway_profiles_schema_mentions_tables_and_indexes() {
        assert!(TABLES.contains(&"model_gateway_profiles"));
        assert!(TABLES.contains(&"model_gateway_profile_events"));
        assert!(MODEL_GATEWAY_PROFILES_SCHEMA
            .sql
            .contains("create table if not exists model_gateway_profiles"));
        assert!(MODEL_GATEWAY_PROFILES_SCHEMA
            .sql
            .contains("unique (tenant_id, profile_id)"));
        assert!(MODEL_GATEWAY_PROFILES_SCHEMA
            .sql
            .contains("create table if not exists model_gateway_profile_events"));
        assert!(MODEL_GATEWAY_PROFILES_SCHEMA
            .sql
            .contains("model_gateway_profiles_lane_enabled_idx"));
        assert!(MODEL_GATEWAY_PROFILES_SCHEMA
            .sql
            .contains("model_gateway_profile_events_profile_created_idx"));
    }

    #[test]
    fn model_gateway_profile_update_defaults_to_no_field_changes() {
        let update = ModelGatewayProfileUpdate::default();

        assert!(update.display_name.is_none());
        assert!(update.enabled.is_none());
        assert!(update.capabilities.is_none());
    }

    #[test]
    fn email_account_auth_schema_mentions_account_tables_and_owners() {
        assert!(TABLES.contains(&"user_sessions"));
        assert!(TABLES.contains(&"email_verification_challenges"));
        assert!(TABLES.contains(&"auth_audit_events"));
        assert!(EMAIL_ACCOUNT_AUTH_SCHEMA
            .sql
            .contains("create table if not exists user_sessions"));
        assert!(EMAIL_ACCOUNT_AUTH_SCHEMA
            .sql
            .contains("create table if not exists email_verification_challenges"));
        assert!(EMAIL_ACCOUNT_AUTH_SCHEMA
            .sql
            .contains("create table if not exists auth_audit_events"));
        assert!(EMAIL_ACCOUNT_AUTH_SCHEMA
            .sql
            .contains("auth_audit_events_user_created_idx"));
        assert!(EMAIL_ACCOUNT_AUTH_SCHEMA
            .sql
            .contains("add column if not exists owner_user_id"));
        assert!(EMAIL_ACCOUNT_AUTH_SCHEMA
            .sql
            .contains("add column if not exists user_id"));
        assert!(MEMORY_DIRECTORY_SCOPE_HARDENING_SCHEMA
            .sql
            .contains("add column if not exists source_document_ids"));
    }

    #[test]
    fn external_integrations_schema_mentions_observe_first_tables() {
        for table in [
            "external_channel_connections",
            "external_source_connections",
            "external_principals",
            "external_permission_snapshots",
            "external_message_events",
            "external_action_runs",
            "external_sync_runs",
        ] {
            assert!(TABLES.contains(&table));
            assert!(EXTERNAL_INTEGRATIONS_SCHEMA
                .sql
                .contains(&format!("create table if not exists {table}")));
        }

        assert!(EXTERNAL_INTEGRATIONS_SCHEMA
            .sql
            .contains("config_redacted jsonb not null"));
        assert!(EXTERNAL_INTEGRATIONS_SCHEMA
            .sql
            .contains("arguments_redacted jsonb not null"));
        assert!(EXTERNAL_INTEGRATIONS_SCHEMA
            .sql
            .contains("references external_source_connections"));
        assert!(EXTERNAL_INTEGRATIONS_SCHEMA
            .sql
            .contains("references external_channel_connections"));
    }

    #[test]
    fn external_integration_config_summary_redacts_secrets_recursively() {
        let summary = external_integration_config_redacted_summary(&json!({
            "base_url": "https://docs.example.com",
            "client_secret": "raw-client-secret",
            "verification_token": "raw-verification-token",
            "nested": {
                "api_key": "raw-api-key",
                "allowed_project": "orders"
            },
            "webhooks": [
                {
                    "signing_key": "raw-signing-key",
                    "name": "ticketing"
                }
            ]
        }));
        let serialized = summary.to_string();

        assert_eq!(summary["base_url"], json!("https://docs.example.com"));
        assert_eq!(summary["client_secret"], json!("[redacted]"));
        assert_eq!(summary["verification_token"], json!("[redacted]"));
        assert_eq!(summary["nested"]["api_key"], json!("[redacted]"));
        assert_eq!(summary["nested"]["allowed_project"], json!("orders"));
        assert_eq!(summary["webhooks"][0]["signing_key"], json!("[redacted]"));
        assert!(!serialized.contains("raw-client-secret"));
        assert!(!serialized.contains("raw-verification-token"));
        assert!(!serialized.contains("raw-api-key"));
        assert!(!serialized.contains("raw-signing-key"));
    }

    #[test]
    fn auth_email_normalization_is_stable() {
        assert_eq!(normalize_email(" User@Example.COM "), "user@example.com");
    }

    #[test]
    fn assistant_run_schema_mentions_run_and_event_tables() {
        assert!(TABLES.contains(&"assistant_runs"));
        assert!(TABLES.contains(&"assistant_run_events"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists assistant_runs"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists assistant_run_events"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("idx_assistant_run_events_run_sequence"));
    }

    #[test]
    fn conversation_memory_schema_mentions_hidden_memory_table() {
        assert!(TABLES.contains(&"conversation_memory_items"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("create table if not exists conversation_memory_items"));
        assert!(INITIAL_SCHEMA
            .sql
            .contains("idx_conversation_memory_thread_updated"));
    }

    #[test]
    fn default_database_url_targets_local_compose_postgres() {
        assert_eq!(
            DEFAULT_LOCAL_DATABASE_URL,
            "postgres://ai_platform:ai_platform@127.0.0.1:5432/ai_data_platform_v3"
        );
    }

    #[test]
    fn local_tenant_defaults_are_stable() {
        assert_eq!(DEFAULT_LOCAL_TENANT_KEY, "local-dev");
        assert_eq!(DEFAULT_LOCAL_TENANT_NAME, "Local Development");
    }

    #[test]
    fn json_object_conversion_keeps_entries() {
        let mut value = Map::new();
        value.insert("scope".to_string(), Value::String("dataset".to_string()));

        let converted = json_object_to_btree_map(Value::Object(value)).expect("object conversion");

        assert_eq!(
            converted.get("scope"),
            Some(&Value::String("dataset".to_string()))
        );
    }

    #[test]
    fn secret_binding_ids_roundtrip_through_document_metadata() {
        let secret_binding_ids = vec![SecretBindingId::new(), SecretBindingId::new()];
        let metadata = document_metadata_with_secret_binding_ids(&secret_binding_ids);

        let parsed =
            secret_binding_ids_from_metadata(&metadata).expect("secret binding ids should parse");

        assert_eq!(parsed, secret_binding_ids);
    }

    #[test]
    fn dataset_metadata_defaults_to_public_visibility() {
        let metadata = dataset_initial_metadata(&Value::Null).expect("metadata should normalize");

        assert_eq!(metadata["visibility"], json!("public"));
        assert_eq!(metadata["default_secret_binding_ids"], json!([]));
        assert_eq!(
            dataset_visibility_from_metadata(&metadata),
            DatasetVisibility::Public
        );
    }

    #[test]
    fn dataset_metadata_preserves_private_secret_bindings() {
        let secret_binding_ids = vec![SecretBindingId::new(), SecretBindingId::new()];
        let metadata = dataset_initial_metadata(&json!({
            "visibility": "private",
            "default_secret_binding_ids": secret_binding_ids,
        }))
        .expect("metadata should normalize");

        assert_eq!(metadata["visibility"], json!("private"));
        assert_eq!(
            dataset_visibility_from_metadata(&metadata),
            DatasetVisibility::Private
        );
        assert_eq!(
            secret_binding_ids_from_metadata_key(&metadata, "default_secret_binding_ids")
                .expect("ids should parse"),
            secret_binding_ids
        );
    }

    #[test]
    fn chunk_metadata_merge_overwrites_existing_keys() {
        let current =
            BTreeMap::from_iter([("retrieval".to_string(), json!({ "state": "pending" }))]);
        let merged = merge_chunk_metadata(
            &current,
            &json!({
                "retrieval": {
                    "state": "indexed",
                    "payload_filter_key": "dataset/demo",
                }
            }),
        )
        .expect("chunk metadata merge should succeed");

        assert_eq!(merged["retrieval"]["state"], json!("indexed"));
        assert_eq!(
            merged["retrieval"]["payload_filter_key"],
            json!("dataset/demo")
        );
    }

    #[test]
    fn retrieval_evidence_ids_helper_preserves_order() {
        let ids = vec![RetrievalEvidenceId::new(), RetrievalEvidenceId::new()];

        let values = retrieval_evidence_ids_to_uuid_array(&ids);

        assert_eq!(values, ids.iter().map(|id| id.0).collect::<Vec<_>>());
    }

    #[test]
    fn retrieval_evidence_insert_is_idempotent_by_execution_and_chunk() {
        assert!(RETRIEVAL_EVIDENCE_UPSERT_SQL
            .contains("on conflict (execution_id, document_chunk_id) do update"));
        assert!(RETRIEVAL_EVIDENCE_UPSERT_SQL
            .contains("where retrieval_evidences.tenant_id = excluded.tenant_id"));
        assert!(RETRIEVAL_EVIDENCE_UPSERT_SQL.contains("returning id, tenant_id"));
    }

    #[test]
    fn parse_manifest_tool_calls_supports_legacy_and_structured_entries() {
        let parsed = parse_manifest_tool_calls(&json!({
            "tool_trace": [
                "legacy.lookup",
                {
                    "call_id": "call_structured",
                    "tool_name": "weather.lookup",
                    "tool": {
                        "key": "weather.lookup",
                        "title": "Weather Lookup",
                        "scope_policy": "session",
                        "invocation_mode": "cli"
                    },
                    "status": "completed",
                    "arguments": { "city": "Shanghai" },
                    "result": { "summary": "sunny" }
                }
            ]
        }))
        .expect("tool trace parses");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].tool_name, "legacy.lookup");
        assert_eq!(parsed[0].status, ToolExecutionStatus::Completed);
        assert_eq!(parsed[1].call_id.as_deref(), Some("call_structured"));
        assert_eq!(parsed[1].tool_name, "weather.lookup");
        assert!(parsed[1].tool_snapshot.is_some());
    }

    #[test]
    fn parse_manifest_llm_invocations_extracts_runtime_payload() {
        let parsed = parse_manifest_llm_invocations(&json!({
            "runtime": {
                "mode": "provider",
                "provider": "openai",
                "model": "gpt-5.4",
                "request_id": "req_123",
                "finish_reason": "tool_calls",
                "latency_ms": 456,
                "usage": {
                    "input_tokens": 17,
                    "output_tokens": 19,
                    "total_tokens": 36
                },
                "system_prompt_key": "chat_session.placeholder",
                "system_prompt_version": "2026-04-19",
                "tool_trace_count": 1
            }
        }))
        .expect("runtime parses");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].mode, LlmInvocationMode::Provider);
        assert_eq!(parsed[0].provider.as_deref(), Some("openai"));
        assert_eq!(parsed[0].request_id.as_deref(), Some("req_123"));
        assert_eq!(
            parsed[0].finish_reason,
            Some(LlmInvocationFinishReason::ToolCalls)
        );
        assert_eq!(parsed[0].latency_ms, Some(456));
        assert_eq!(
            parsed[0].usage.as_ref().map(|usage| usage.total_tokens),
            Some(36)
        );
        assert_eq!(parsed[0].tool_trace_count, Some(1));
    }
}
