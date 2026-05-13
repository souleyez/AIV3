use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{TenantId, WorkflowExecution, WorkflowTask};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use serde_json::{json, Map, Value};
use storage::{PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "external_source";
const DEFAULT_WAKE_TASK_KEY: &str = "sync_external_users";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

const SYNC_USERS_TASK_KEY: &str = "sync_external_users";
const SYNC_ACL_TASK_KEY: &str = "sync_external_acl";
const SYNC_METADATA_TASK_KEY: &str = "sync_external_metadata";
const FETCH_CONTENT_TASK_KEY: &str = "fetch_external_content";

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("external_source_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue =
        std::env::var("EXTERNAL_SOURCE_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key = optional_env("EXTERNAL_SOURCE_TASK_KEY");
    let poll_interval = std::env::var("EXTERNAL_SOURCE_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let wake_task_key = task_key.as_deref().unwrap_or(DEFAULT_WAKE_TASK_KEY);
    let wake_subject = workflow_task_enqueued_subject(&queue, wake_task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("external_source_worker.{queue}.{wake_task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "external-source-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, task_key.as_deref(), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, task).await
                {
                    tracing::error!(error = ?error, "external source task processing failed");
                }
            }
            Ok(None) => wait_for_next_task_signal(&mut task_waker, poll_interval).await,
            Err(error) => {
                tracing::error!(error = ?error, "external source worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    task: WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    let result = match task.task_key.as_str() {
        SYNC_USERS_TASK_KEY => sync_external_users(storage, &execution).await,
        SYNC_ACL_TASK_KEY => sync_external_acl(storage, &execution).await,
        SYNC_METADATA_TASK_KEY => sync_external_metadata(&execution),
        FETCH_CONTENT_TASK_KEY => fetch_external_content(&execution),
        unknown => Err(anyhow!("unsupported external source task key: {unknown}")),
    };

    match result {
        Ok(output) => {
            platform_api::apply_workflow_signal_with_dependencies(
                storage,
                workflow_catalog,
                event_bus,
                task.tenant_id,
                task.execution_id,
                WorkflowSignal::StepCompleted {
                    task_key: task.task_key.clone(),
                    output: Some(output),
                },
            )
            .await?;
            storage
                .workflow_tasks()
                .mark_succeeded(task.id, Utc::now())
                .await?;
            tracing::info!(
                task_id = %task.id,
                execution_id = %task.execution_id,
                task_key = %task.task_key,
                "external source task completed"
            );
            Ok(())
        }
        Err(error) => {
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
                    "external source worker failed to send workflow step_failed signal"
                );
            }
            storage
                .workflow_tasks()
                .mark_failed(task.id, &error_message, Utc::now())
                .await?;
            Err(error)
        }
    }
}

async fn sync_external_users(storage: &PgStorage, execution: &WorkflowExecution) -> Result<Value> {
    let connector = connector_fixture(&execution.context)?;
    let platform = connector_platform(&connector);
    let users = value_array(connector.get("users"));

    for user in &users {
        let external_user_id = required_string(user, &["external_user_id", "externalUserId"])?;
        let trust_level = string_field(user, &["trust_level", "trustLevel"])
            .unwrap_or_else(|| "external_user".to_string());
        let is_disabled = user
            .get("is_disabled")
            .or_else(|| user.get("isDisabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let v3_user_id = string_field(user, &["v3_user_id", "v3UserId"])
            .and_then(|raw| uuid::Uuid::parse_str(&raw).ok());
        let departments = json_array_or_empty(user, &["external_department_ids", "department_ids"]);
        let groups = json_array_or_empty(user, &["external_group_ids", "group_ids"]);
        let roles = json_array_or_empty(user, &["external_role_ids", "role_ids"]);
        let profile_redacted = object_field(user, &["profile_redacted", "profileRedacted"])
            .unwrap_or_else(|| json!({}));

        sqlx::query(
            r#"
            insert into external_principals (
                tenant_id,
                platform,
                external_user_id,
                v3_user_id,
                trust_level,
                is_disabled,
                external_department_ids,
                external_group_ids,
                external_role_ids,
                profile_redacted
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            on conflict (tenant_id, platform, external_user_id)
            do update set
                v3_user_id = excluded.v3_user_id,
                trust_level = excluded.trust_level,
                is_disabled = excluded.is_disabled,
                external_department_ids = excluded.external_department_ids,
                external_group_ids = excluded.external_group_ids,
                external_role_ids = excluded.external_role_ids,
                profile_redacted = excluded.profile_redacted,
                updated_at = now()
            "#,
        )
        .bind(execution.tenant_id.0)
        .bind(&platform)
        .bind(external_user_id)
        .bind(v3_user_id)
        .bind(trust_level)
        .bind(is_disabled)
        .bind(departments)
        .bind(groups)
        .bind(roles)
        .bind(profile_redacted)
        .execute(storage.pool())
        .await?;
    }

    let output = json!({
        "source_id": context_required_string(&execution.context, "source_id")?,
        "external_sync_run_id": context_string(&execution.context, "external_sync_run_id")?,
        "platform": platform,
        "user_count": users.len(),
        "group_count": value_array(connector.get("groups")).len(),
    });
    update_external_sync_counts(storage, execution.tenant_id, &output).await?;
    Ok(output)
}

async fn sync_external_acl(storage: &PgStorage, execution: &WorkflowExecution) -> Result<Value> {
    let connector = connector_fixture(&execution.context)?;
    let source_id = context_required_string(&execution.context, "source_id")?;
    let snapshots = acl_snapshots_from_connector(&connector)?;

    for snapshot in &snapshots {
        upsert_external_permission_snapshot(storage, execution.tenant_id, &source_id, snapshot)
            .await?;
    }

    let output = json!({
        "source_id": source_id,
        "external_sync_run_id": context_string(&execution.context, "external_sync_run_id")?,
        "acl_snapshot_count": snapshots.len(),
    });
    update_external_sync_counts(storage, execution.tenant_id, &output).await?;
    Ok(output)
}

fn sync_external_metadata(execution: &WorkflowExecution) -> Result<Value> {
    let connector = connector_fixture(&execution.context)?;
    let documents = external_document_metadata_from_connector(&connector)?;

    Ok(json!({
        "source_id": context_required_string(&execution.context, "source_id")?,
        "external_sync_run_id": context_string(&execution.context, "external_sync_run_id")?,
        "document_count": documents.len(),
        "external_document_metadata": documents,
    }))
}

fn fetch_external_content(execution: &WorkflowExecution) -> Result<Value> {
    let connector = connector_fixture(&execution.context)?;
    let documents = external_documents_from_connector(&connector)?;

    Ok(json!({
        "source_id": context_required_string(&execution.context, "source_id")?,
        "external_sync_run_id": context_string(&execution.context, "external_sync_run_id")?,
        "document_count": documents.len(),
        "external_documents": documents,
    }))
}

#[derive(Clone, Debug)]
struct ExternalAclSnapshotInput {
    document_external_id: String,
    revision_external_id: Option<String>,
    acl_snapshot: Value,
    acl_hash: Option<String>,
}

fn connector_fixture(context: &Value) -> Result<Value> {
    let connector_context = context
        .get("connector_context")
        .filter(|value| value.is_object())
        .ok_or_else(|| anyhow!("external source workflow missing connector_context"))?;
    Ok(connector_context
        .get("mock_https_connector")
        .or_else(|| connector_context.get("mockHttpsConnector"))
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| connector_context.clone()))
}

fn connector_platform(connector: &Value) -> String {
    string_field(connector, &["platform"]).unwrap_or_else(|| "third_party".to_string())
}

fn external_document_metadata_from_connector(connector: &Value) -> Result<Vec<Value>> {
    external_documents_from_connector(connector).map(|documents| {
        documents
            .into_iter()
            .map(|document| {
                let mut object = document.as_object().cloned().unwrap_or_default();
                object.remove("body");
                object.remove("content");
                object.remove("text");
                Value::Object(object)
            })
            .collect()
    })
}

fn external_documents_from_connector(connector: &Value) -> Result<Vec<Value>> {
    let documents = value_array(connector.get("documents"));
    if documents.is_empty() {
        return Err(anyhow!("mock HTTPS connector requires documents array"));
    }

    documents
        .iter()
        .map(|document| {
            let document_external_id =
                required_string(document, &["document_external_id", "documentExternalId"])?;
            let body = string_field(document, &["body", "content", "text"])
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    anyhow!("external document {document_external_id} missing non-empty body")
                })?;
            let mut object = document.as_object().cloned().unwrap_or_default();
            object.insert(
                "document_external_id".to_string(),
                json!(document_external_id),
            );
            object.insert(
                "revision_external_id".to_string(),
                json!(string_field(
                    document,
                    &["revision_external_id", "revisionExternalId"]
                )),
            );
            object.insert(
                "title".to_string(),
                json!(string_field(document, &["title", "name"])
                    .unwrap_or_else(|| "Untitled external document".to_string())),
            );
            object.insert(
                "content_type".to_string(),
                json!(string_field(document, &["content_type", "contentType"])
                    .unwrap_or_else(|| "text/markdown".to_string())),
            );
            object.insert("body".to_string(), json!(body));
            if let Some(acl) = matching_acl_snapshot(connector, document)? {
                object.insert("acl_snapshot".to_string(), acl.acl_snapshot);
                object.insert("acl_hash".to_string(), json!(acl.acl_hash));
            }
            Ok(Value::Object(object))
        })
        .collect()
}

fn acl_snapshots_from_connector(connector: &Value) -> Result<Vec<ExternalAclSnapshotInput>> {
    let snapshots = value_array(
        connector
            .get("acl_snapshots")
            .or_else(|| connector.get("aclSnapshots")),
    );

    if !snapshots.is_empty() {
        return snapshots.iter().map(acl_snapshot_from_value).collect();
    }

    value_array(connector.get("documents"))
        .iter()
        .filter(|document| {
            document.get("acl_snapshot").is_some()
                || document.get("aclSnapshot").is_some()
                || document.get("acl").is_some()
        })
        .map(acl_snapshot_from_value)
        .collect()
}

fn matching_acl_snapshot(
    connector: &Value,
    document: &Value,
) -> Result<Option<ExternalAclSnapshotInput>> {
    let document_external_id =
        required_string(document, &["document_external_id", "documentExternalId"])?;
    let revision_external_id =
        string_field(document, &["revision_external_id", "revisionExternalId"]);
    let snapshots = acl_snapshots_from_connector(connector)?;
    Ok(snapshots.into_iter().find(|snapshot| {
        snapshot.document_external_id == document_external_id
            && (snapshot.revision_external_id.is_none()
                || snapshot.revision_external_id == revision_external_id)
    }))
}

fn acl_snapshot_from_value(value: &Value) -> Result<ExternalAclSnapshotInput> {
    let document_external_id =
        required_string(value, &["document_external_id", "documentExternalId"])?;
    let revision_external_id = string_field(value, &["revision_external_id", "revisionExternalId"]);
    let acl_snapshot = value
        .get("acl_snapshot")
        .or_else(|| value.get("aclSnapshot"))
        .or_else(|| value.get("acl"))
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "allowed_user_external_ids": json_array_or_empty(value, &["allowed_user_external_ids", "allowedUserExternalIds"]),
                "allowed_department_external_ids": json_array_or_empty(value, &["allowed_department_external_ids", "allowedDepartmentExternalIds"]),
                "allowed_group_external_ids": json_array_or_empty(value, &["allowed_group_external_ids", "allowedGroupExternalIds"]),
                "allowed_role_external_ids": json_array_or_empty(value, &["allowed_role_external_ids", "allowedRoleExternalIds"]),
                "denied_user_external_ids": json_array_or_empty(value, &["denied_user_external_ids", "deniedUserExternalIds"]),
                "denied_department_external_ids": json_array_or_empty(value, &["denied_department_external_ids", "deniedDepartmentExternalIds"]),
                "denied_group_external_ids": json_array_or_empty(value, &["denied_group_external_ids", "deniedGroupExternalIds"]),
                "denied_role_external_ids": json_array_or_empty(value, &["denied_role_external_ids", "deniedRoleExternalIds"]),
            })
        });

    Ok(ExternalAclSnapshotInput {
        document_external_id,
        revision_external_id,
        acl_snapshot,
        acl_hash: string_field(value, &["acl_hash", "aclHash"]),
    })
}

async fn upsert_external_permission_snapshot(
    storage: &PgStorage,
    tenant_id: TenantId,
    source_id: &str,
    snapshot: &ExternalAclSnapshotInput,
) -> Result<()> {
    let now = Utc::now();
    let updated = sqlx::query(
        r#"
        update external_permission_snapshots
        set acl_snapshot = $5,
            acl_hash = $6,
            captured_at = $7
        where tenant_id = $1
          and source_id = $2
          and document_external_id = $3
          and coalesce(revision_external_id, '') = coalesce($4::text, '')
        "#,
    )
    .bind(tenant_id.0)
    .bind(source_id)
    .bind(&snapshot.document_external_id)
    .bind(&snapshot.revision_external_id)
    .bind(&snapshot.acl_snapshot)
    .bind(&snapshot.acl_hash)
    .bind(now)
    .execute(storage.pool())
    .await?;

    if updated.rows_affected() == 0 {
        sqlx::query(
            r#"
            insert into external_permission_snapshots (
                tenant_id,
                source_id,
                document_external_id,
                revision_external_id,
                acl_snapshot,
                acl_hash,
                captured_at
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(tenant_id.0)
        .bind(source_id)
        .bind(&snapshot.document_external_id)
        .bind(&snapshot.revision_external_id)
        .bind(&snapshot.acl_snapshot)
        .bind(&snapshot.acl_hash)
        .bind(now)
        .execute(storage.pool())
        .await?;
    }

    Ok(())
}

async fn update_external_sync_counts(
    storage: &PgStorage,
    tenant_id: TenantId,
    output: &Value,
) -> Result<()> {
    let Some(sync_run_id) = output
        .get("external_sync_run_id")
        .and_then(Value::as_str)
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
    else {
        return Ok(());
    };

    let mut counts = Map::new();
    for key in [
        "user_count",
        "group_count",
        "acl_snapshot_count",
        "document_count",
    ] {
        if let Some(value) = output.get(key).and_then(Value::as_u64) {
            counts.insert(key.to_string(), json!(value));
        }
    }
    if counts.is_empty() {
        return Ok(());
    }

    sqlx::query(
        r#"
        update external_sync_runs
        set counts = counts || $3,
            updated_at = $4
        where tenant_id = $1 and id = $2
        "#,
    )
    .bind(tenant_id.0)
    .bind(sync_run_id)
    .bind(Value::Object(counts))
    .bind(Utc::now())
    .execute(storage.pool())
    .await?;

    Ok(())
}

fn value_array(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn json_array_or_empty(value: &Value, keys: &[&str]) -> Value {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_array))
        .map(|items| Value::Array(items.clone()))
        .unwrap_or_else(|| json!([]))
}

fn object_field(value: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter()
        .find_map(|key| value.get(*key).filter(|candidate| candidate.is_object()))
        .cloned()
}

fn required_string(value: &Value, keys: &[&str]) -> Result<String> {
    string_field(value, keys)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("missing required string field: {}", keys.join("|")))
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToString::to_string)
}

fn context_required_string(value: &Value, key: &str) -> Result<String> {
    context_string(value, key)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("workflow execution context missing {key}"))
}

fn context_string(value: &Value, key: &str) -> Result<Option<String>> {
    match value {
        Value::Object(map) => Ok(map
            .get(key)
            .and_then(Value::as_str)
            .map(ToString::to_string)),
        Value::Null => Ok(None),
        _ => Err(anyhow!("workflow execution context must be a JSON object")),
    }
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "*")
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "external source worker received task wake signal");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> Value {
        json!({
            "source_id": "src-docs",
            "external_sync_run_id": uuid::Uuid::new_v4().to_string(),
            "connector_context": {
                "mock_https_connector": {
                    "platform": "generic_chat",
                    "users": [{
                        "external_user_id": "user-a",
                        "trust_level": "employee",
                        "external_group_ids": ["group-risk"]
                    }],
                    "groups": [{
                        "external_group_id": "group-risk"
                    }],
                    "acl_snapshots": [{
                        "document_external_id": "doc-001",
                        "revision_external_id": "rev-1",
                        "allowed_group_external_ids": ["group-risk"],
                        "denied_user_external_ids": ["user-blocked"],
                        "acl_hash": "acl-001"
                    }],
                    "documents": [{
                        "document_external_id": "doc-001",
                        "revision_external_id": "rev-1",
                        "title": "风险制度",
                        "content_type": "text/markdown",
                        "body": "订单延期超过两天需要升级处理。",
                        "metadata": {
                            "path": "/risk/doc-001"
                        }
                    }]
                }
            }
        })
    }

    #[test]
    fn connector_fixture_accepts_mock_https_connector_shape() {
        let connector = connector_fixture(&sample_context()).expect("connector fixture");

        assert_eq!(connector_platform(&connector), "generic_chat");
        assert_eq!(value_array(connector.get("users")).len(), 1);
        assert_eq!(value_array(connector.get("documents")).len(), 1);
    }

    #[test]
    fn acl_snapshots_can_be_built_from_direct_acl_fields() {
        let connector = connector_fixture(&sample_context()).expect("connector fixture");
        let snapshots = acl_snapshots_from_connector(&connector).expect("ACL snapshots");

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].document_external_id, "doc-001");
        assert_eq!(snapshots[0].revision_external_id.as_deref(), Some("rev-1"));
        assert_eq!(
            snapshots[0].acl_snapshot["allowed_group_external_ids"],
            json!(["group-risk"])
        );
        assert_eq!(
            snapshots[0].acl_snapshot["denied_user_external_ids"],
            json!(["user-blocked"])
        );
    }

    #[test]
    fn fetch_content_output_includes_body_and_matching_acl_snapshot() {
        let execution = WorkflowExecution {
            id: domain_model::WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: Some(domain_model::DatasetId::new()),
            report_plan_id: None,
            kind: domain_model::WorkflowKind::ExternalSourceSync,
            version: "0.1.0".to_string(),
            stage: "fetch_content".to_string(),
            status: domain_model::WorkflowStatus::Running,
            attempt: 0,
            context: sample_context(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let output = fetch_external_content(&execution).expect("content output");

        assert_eq!(output["document_count"], json!(1));
        assert_eq!(
            output["external_documents"][0]["document_external_id"],
            json!("doc-001")
        );
        assert_eq!(
            output["external_documents"][0]["body"],
            json!("订单延期超过两天需要升级处理。")
        );
        assert_eq!(
            output["external_documents"][0]["acl_snapshot"]["allowed_group_external_ids"],
            json!(["group-risk"])
        );
    }

    #[test]
    fn metadata_output_redacts_body_before_workflow_event() {
        let execution = WorkflowExecution {
            id: domain_model::WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: Some(domain_model::DatasetId::new()),
            report_plan_id: None,
            kind: domain_model::WorkflowKind::ExternalSourceSync,
            version: "0.1.0".to_string(),
            stage: "sync_metadata".to_string(),
            status: domain_model::WorkflowStatus::Running,
            attempt: 0,
            context: sample_context(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let output = sync_external_metadata(&execution).expect("metadata output");

        assert_eq!(output["document_count"], json!(1));
        assert!(output["external_document_metadata"][0]
            .get("body")
            .is_none());
        assert!(output["external_document_metadata"][0]
            .get("content")
            .is_none());
        assert!(output["external_document_metadata"][0]
            .get("text")
            .is_none());
    }
}
