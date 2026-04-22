use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use domain_model::{
    ChatMessage, ChatMessageId, ChatMessageRole, ChatSession, ChatSessionId, Dataset, DatasetId,
    DatasetLifecycle, DatasetOutput, DatasetOutputId, Document, DocumentChunk, DocumentChunkId,
    DocumentChunkState, DocumentId, DocumentLifecycle, LlmInvocation, LlmInvocationFinishReason,
    LlmInvocationId, LlmInvocationMode, LlmInvocationSourceKind, LlmTokenUsage, MemoryDirectory,
    MemoryDirectoryId, PublishedReport, PublishedReportId, PublishedReportVersion,
    PublishedReportVersionId, ReportPlan, ReportPlanAstVersion, ReportPlanAstVersionId,
    ReportPlanId, ReportPlanStatus, ReportRenderOutput, ReportRenderOutputId,
    ReportRenderOutputStatus, RetrievalEvidence, RetrievalEvidenceId, SecretBindingId, Tenant,
    TenantId, ToolExecution, ToolExecutionId, ToolExecutionSourceKind, ToolExecutionStatus,
    WorkflowEventId, WorkflowEventRecord, WorkflowExecution, WorkflowExecutionId, WorkflowKind,
    WorkflowStatus, WorkflowTask, WorkflowTaskId, WorkflowTaskStatus,
};
use serde_json::{Map, Value};
use sqlx::{postgres::PgPoolOptions, Executor, PgPool, Row};
use std::collections::BTreeMap;
use std::time::Duration;
use uuid::Uuid;
use workflow_engine::WorkflowDefinitionSummary;

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

pub const TABLES: &[&str] = &[
    "tenants",
    "users",
    "datasets",
    "documents",
    "document_chunks",
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
    "chat_sessions",
    "chat_messages",
    "llm_invocations",
    "tool_executions",
    "report_modules",
    "published_reports",
    "published_report_versions",
];

pub const DEFAULT_LOCAL_DATABASE_URL: &str =
    "postgres://ai_platform:ai_platform@127.0.0.1:5432/ai_data_platform_v3";
pub const DEFAULT_LOCAL_TENANT_KEY: &str = "local-dev";
pub const DEFAULT_LOCAL_TENANT_NAME: &str = "Local Development";

#[derive(Clone, Debug)]
pub struct NewDataset {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewDocument {
    pub dataset_id: DatasetId,
    pub title: String,
    pub object_key: String,
    pub content_type: String,
    pub secret_binding_ids: Vec<SecretBindingId>,
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
pub struct NewReportPlan {
    pub dataset_id: DatasetId,
    pub title: String,
    pub objective: String,
    pub theme_key: String,
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
pub struct NewMemoryDirectory {
    pub execution_id: WorkflowExecutionId,
    pub dataset_id: DatasetId,
    pub directory_nodes: i32,
    pub refreshed_chunks: i32,
    pub directory_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDatasetOutput {
    pub execution_id: WorkflowExecutionId,
    pub dataset_id: DatasetId,
    pub prompt: String,
    pub output_text: String,
    pub memory_directory_id: Option<MemoryDirectoryId>,
    pub retrieval_evidence_ids: Vec<RetrievalEvidenceId>,
    pub output_manifest: Value,
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
pub struct NewChatSession {
    pub id: ChatSessionId,
    pub execution_id: WorkflowExecutionId,
    pub dataset_id: DatasetId,
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
        Self::connect_with_settings(database_url, 10, Duration::from_secs(30)).await
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
        sqlx::raw_sql(INITIAL_SCHEMA.sql)
            .execute(&self.pool)
            .await?;
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

    pub fn documents(&self) -> PgDocumentRepository {
        PgDocumentRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn document_chunks(&self) -> PgDocumentChunkRepository {
        PgDocumentChunkRepository {
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

    pub fn dataset_outputs(&self) -> PgDatasetOutputRepository {
        PgDatasetOutputRepository {
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

#[derive(Clone)]
pub struct PgDatasetRepository {
    pool: PgPool,
}

impl PgDatasetRepository {
    pub async fn create(&self, tenant_id: TenantId, new_dataset: NewDataset) -> Result<Dataset> {
        let row = sqlx::query(
            r#"
            insert into datasets (tenant_id, key, title, description, lifecycle, metadata)
            values ($1, $2, $3, $4, $5, '{}'::jsonb)
            returning id, tenant_id, key, title, description, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_dataset.key)
        .bind(new_dataset.title)
        .bind(new_dataset.description)
        .bind(DatasetLifecycle::Draft.as_str())
        .fetch_one(&self.pool)
        .await?;

        map_dataset_row(&row)
    }

    pub async fn list_by_tenant(&self, tenant_id: TenantId) -> Result<Vec<Dataset>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, key, title, description, lifecycle, metadata, created_at, updated_at
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
            select id, tenant_id, key, title, description, lifecycle, metadata, created_at, updated_at
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
}

#[derive(Clone)]
pub struct PgDocumentRepository {
    pool: PgPool,
}

impl PgDocumentRepository {
    pub async fn create(&self, tenant_id: TenantId, new_document: NewDocument) -> Result<Document> {
        let row = sqlx::query(
            r#"
            insert into documents (
                tenant_id,
                dataset_id,
                title,
                object_key,
                content_type,
                lifecycle,
                metadata
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            returning id, tenant_id, dataset_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_document.dataset_id.0)
        .bind(new_document.title)
        .bind(new_document.object_key)
        .bind(new_document.content_type)
        .bind(DocumentLifecycle::Received.as_str())
        .bind(document_metadata_with_secret_binding_ids(
            &new_document.secret_binding_ids,
        ))
        .fetch_one(&self.pool)
        .await?;

        map_document_row(&row)
    }

    pub async fn list_by_tenant(&self, tenant_id: TenantId) -> Result<Vec<Document>> {
        let rows = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
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
            select id, tenant_id, dataset_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
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

    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        document_id: DocumentId,
    ) -> Result<Option<Document>> {
        let row = sqlx::query(
            r#"
            select id, tenant_id, dataset_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
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
            returning id, tenant_id, dataset_id, title, object_key, content_type, lifecycle, metadata, created_at, updated_at
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
pub struct PgReportPlanRepository {
    pool: PgPool,
}

impl PgReportPlanRepository {
    pub async fn create(&self, tenant_id: TenantId, new_plan: NewReportPlan) -> Result<ReportPlan> {
        let row = sqlx::query(
            r#"
            insert into report_plans (tenant_id, dataset_id, title, objective, status, theme_key, ast)
            values ($1, $2, $3, $4, $5, $6, '{}'::jsonb)
            returning id, tenant_id, dataset_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
            "#,
        )
        .bind(tenant_id.0)
        .bind(new_plan.dataset_id.0)
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
            select id, tenant_id, dataset_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
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
            select id, tenant_id, dataset_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
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
            returning id, tenant_id, dataset_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
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
            returning id, tenant_id, dataset_id, title, objective, status, theme_key, current_ast_version_id, created_at, updated_at
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
                next_version.version_no,
                $5,
                $6,
                jsonb_set(
                    jsonb_set($7, '{version_no}', to_jsonb(next_version.version_no), true),
                    '{root,version_no}',
                    to_jsonb(next_version.version_no),
                    true
                ),
                $8
            from next_version
            returning id, tenant_id, dataset_id, execution_id, version_no, directory_nodes,
                      refreshed_chunks, directory_manifest, created_at
            "#,
        )
        .bind(MemoryDirectoryId::new().0)
        .bind(tenant_id.0)
        .bind(new_directory.dataset_id.0)
        .bind(new_directory.execution_id.0)
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
            select id, tenant_id, dataset_id, execution_id, version_no, directory_nodes, refreshed_chunks, directory_manifest, created_at
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
            select id, tenant_id, dataset_id, execution_id, version_no, directory_nodes, refreshed_chunks, directory_manifest, created_at
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
pub struct PgRetrievalEvidenceRepository {
    pool: PgPool,
}

impl PgRetrievalEvidenceRepository {
    pub async fn create_many(
        &self,
        tenant_id: TenantId,
        evidences: &[NewRetrievalEvidence],
    ) -> Result<Vec<RetrievalEvidence>> {
        let mut tx = self.pool.begin().await?;
        let mut persisted = Vec::with_capacity(evidences.len());

        for evidence in evidences {
            let row = sqlx::query(
                r#"
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
                    created_at
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                returning id, tenant_id, dataset_id, execution_id, document_id, document_chunk_id,
                          chunk_index, source_locator, content_excerpt, summary, payload_filter_key,
                          embedding_model, recall_score, evidence_manifest, created_at
                "#,
            )
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
                where tenant_id = $1 and dataset_id = $2
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
                prompt,
                output_text,
                memory_directory_id,
                retrieval_evidence_ids,
                output_manifest,
                created_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            returning id, tenant_id, execution_id, dataset_id, prompt, output_text,
                      memory_directory_id, retrieval_evidence_ids, output_manifest, created_at
            "#,
        )
        .bind(DatasetOutputId::new().0)
        .bind(tenant_id.0)
        .bind(new_output.execution_id.0)
        .bind(new_output.dataset_id.0)
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
                   memory_directory_id, retrieval_evidence_ids, output_manifest, created_at
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
                   memory_directory_id, retrieval_evidence_ids, output_manifest, created_at
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
                   memory_directory_id, retrieval_evidence_ids, output_manifest, created_at
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
                execution_id,
                title,
                latest_memory_directory_id,
                latest_dataset_output_id,
                session_manifest,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            returning id, tenant_id, dataset_id, execution_id, title, latest_memory_directory_id,
                      latest_dataset_output_id, session_manifest, created_at, updated_at
            "#,
        )
        .bind(new_session.id.0)
        .bind(tenant_id.0)
        .bind(new_session.dataset_id.0)
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
            select id, tenant_id, dataset_id, execution_id, title, latest_memory_directory_id,
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
            select id, tenant_id, dataset_id, execution_id, title, latest_memory_directory_id,
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
            select id, tenant_id, dataset_id, execution_id, title, latest_memory_directory_id,
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
            returning id, tenant_id, dataset_id, execution_id, title, latest_memory_directory_id,
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
}

fn map_dataset_row(row: &sqlx::postgres::PgRow) -> Result<Dataset> {
    let lifecycle = row.get::<String, _>("lifecycle");
    let metadata = row.get::<Value, _>("metadata");

    Ok(Dataset {
        id: DatasetId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        key: row.get("key"),
        title: row.get("title"),
        description: row.get("description"),
        lifecycle: DatasetLifecycle::from_str(&lifecycle)
            .ok_or_else(|| anyhow!("unknown dataset lifecycle: {lifecycle}"))?,
        default_secret_binding_ids: Vec::new(),
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

fn map_report_plan_row(row: &sqlx::postgres::PgRow) -> Result<ReportPlan> {
    let status = row.get::<String, _>("status");

    Ok(ReportPlan {
        id: ReportPlanId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
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

fn map_memory_directory_row(row: &sqlx::postgres::PgRow) -> Result<MemoryDirectory> {
    Ok(MemoryDirectory {
        id: MemoryDirectoryId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
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

fn map_dataset_output_row(row: &sqlx::postgres::PgRow) -> Result<DatasetOutput> {
    Ok(DatasetOutput {
        id: DatasetOutputId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        execution_id: WorkflowExecutionId(row.get::<Uuid, _>("execution_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
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

fn map_chat_session_row(row: &sqlx::postgres::PgRow) -> Result<ChatSession> {
    Ok(ChatSession {
        id: ChatSessionId(row.get::<Uuid, _>("id")),
        tenant_id: TenantId(row.get::<Uuid, _>("tenant_id")),
        dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
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

fn secret_binding_ids_from_metadata(metadata: &Value) -> Result<Vec<SecretBindingId>> {
    let Some(entries) = metadata
        .as_object()
        .and_then(|map| map.get("secret_binding_ids"))
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };

    entries
        .iter()
        .map(|entry| {
            let raw = entry
                .as_str()
                .ok_or_else(|| anyhow!("secret_binding_ids entries must be strings"))?;
            Uuid::parse_str(raw)
                .map(SecretBindingId)
                .map_err(|error| anyhow!("invalid secret binding id {raw}: {error}"))
        })
        .collect()
}

fn retrieval_evidence_ids_to_uuid_array(ids: &[RetrievalEvidenceId]) -> Vec<Uuid> {
    ids.iter().map(|id| id.0).collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map};

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
