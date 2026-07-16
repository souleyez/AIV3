use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{TenantId, WorkflowExecution, WorkflowTask};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use external_source_connectors::{
    fetch_mysql_documents_report_with_checkpoint, MySqlSourceCheckpoint, MySqlSourceConfig,
};
use reqwest::blocking::Client;
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, time::Duration as StdDuration};
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
const DEFAULT_HTTP_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_HTTP_DOCUMENT_LIMIT: usize = 100;
const DEFAULT_HTTP_MAX_PAGES: usize = 20;

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

    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "EXTERNAL_SOURCE_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
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
        database_endpoint = %observability::redact_connection_endpoint(&database_url),
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
        SYNC_METADATA_TASK_KEY => sync_external_metadata(storage, &execution).await,
        FETCH_CONTENT_TASK_KEY => fetch_external_content(storage, &execution).await,
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
            update_external_sync_failure_summary(
                storage,
                task.tenant_id,
                &execution.context,
                &task.task_key,
                &error_message,
            )
            .await?;
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
    let connector = connector_fixture_for(&execution.context, ConnectorFetchScope::Users).await?;
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
            .or_else(|| {
                string_field(user, &["status"]).map(|status| {
                    !matches!(
                        status.trim().to_ascii_lowercase().as_str(),
                        "active" | "enabled" | "normal"
                    )
                })
            })
            .unwrap_or(false);
        let v3_user_id = string_field(user, &["v3_user_id", "v3UserId"])
            .and_then(|raw| uuid::Uuid::parse_str(&raw).ok());
        let departments = json_array_or_empty(
            user,
            &[
                "external_department_ids",
                "department_external_ids",
                "department_ids",
            ],
        );
        let groups = json_array_or_empty(
            user,
            &["external_group_ids", "group_external_ids", "group_ids"],
        );
        let roles = json_array_or_empty(
            user,
            &["external_role_ids", "role_external_ids", "role_ids"],
        );
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
    let connector = connector_fixture_for(&execution.context, ConnectorFetchScope::Acl).await?;
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

async fn sync_external_metadata(
    storage: &PgStorage,
    execution: &WorkflowExecution,
) -> Result<Value> {
    let output = build_sync_external_metadata_output(execution).await?;
    update_external_sync_counts(storage, execution.tenant_id, &output).await?;
    Ok(output)
}

async fn build_sync_external_metadata_output(execution: &WorkflowExecution) -> Result<Value> {
    let connector =
        connector_fixture_for(&execution.context, ConnectorFetchScope::Metadata).await?;
    let documents = external_document_metadata_from_connector(&connector)?;
    let fetch_summary =
        connector_database_fetch_summary(&connector, &documents, "metadata_document_count");

    Ok(json!({
        "source_id": context_required_string(&execution.context, "source_id")?,
        "external_sync_run_id": context_string(&execution.context, "external_sync_run_id")?,
        "document_count": documents.len(),
        "row_count": fetch_summary.row_count,
        "metadata_document_count": documents.len(),
        "metadata_row_count": fetch_summary.row_count,
        "skipped_row_count": fetch_summary.skipped_row_count,
        "failed_row_count": fetch_summary.failed_row_count,
        "row_failure_samples": fetch_summary.row_failure_samples,
        "metadata_table_counts": fetch_summary.table_counts,
        "next_checkpoint": connector
            .get("next_checkpoint")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| json!({})),
        "external_document_metadata": documents,
    }))
}

async fn fetch_external_content(
    storage: &PgStorage,
    execution: &WorkflowExecution,
) -> Result<Value> {
    let output = build_fetch_external_content_output(execution).await?;
    update_external_sync_counts(storage, execution.tenant_id, &output).await?;
    Ok(output)
}

async fn build_fetch_external_content_output(execution: &WorkflowExecution) -> Result<Value> {
    let connector = connector_fixture_for(&execution.context, ConnectorFetchScope::Content).await?;
    let documents = external_documents_from_connector(&connector)?;
    let fetch_summary =
        connector_database_fetch_summary(&connector, &documents, "content_document_count");

    Ok(json!({
        "source_id": context_required_string(&execution.context, "source_id")?,
        "external_sync_run_id": context_string(&execution.context, "external_sync_run_id")?,
        "document_count": documents.len(),
        "row_count": fetch_summary.row_count,
        "content_document_count": documents.len(),
        "content_row_count": fetch_summary.row_count,
        "skipped_row_count": fetch_summary.skipped_row_count,
        "failed_row_count": fetch_summary.failed_row_count,
        "row_failure_samples": fetch_summary.row_failure_samples,
        "content_table_counts": fetch_summary.table_counts,
        "next_checkpoint": connector
            .get("next_checkpoint")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| json!({})),
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

#[derive(Clone, Copy, Debug)]
enum ConnectorFetchScope {
    Users,
    Acl,
    Metadata,
    Content,
}

#[cfg(test)]
async fn connector_fixture(context: &Value) -> Result<Value> {
    connector_fixture_for(context, ConnectorFetchScope::Content).await
}

async fn connector_fixture_for(context: &Value, scope: ConnectorFetchScope) -> Result<Value> {
    let connector_context = context
        .get("connector_context")
        .filter(|value| value.is_object())
        .ok_or_else(|| anyhow!("external source workflow missing connector_context"))?;
    if let Some(mysql_source) = mysql_source_config_value(connector_context) {
        let checkpoint = context.get("checkpoint").unwrap_or(&Value::Null);
        return fetch_mysql_source_connector(mysql_source, checkpoint, scope).await;
    }
    if let Some(http_source) = http_source_config_value(connector_context) {
        let context = context.clone();
        let http_source = http_source.clone();
        return tokio::task::spawn_blocking(move || {
            fetch_http_source_connector(&context, &http_source, scope)
        })
        .await
        .map_err(|error| anyhow!("http_source connector join failed: {error}"))?;
    }
    Ok(connector_context
        .get("mock_https_connector")
        .or_else(|| connector_context.get("mockHttpsConnector"))
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| connector_context.clone()))
}

fn mysql_source_config_value(connector_context: &Value) -> Option<&Value> {
    connector_context
        .get("mysql_source")
        .or_else(|| connector_context.get("mysqlSource"))
        .or_else(|| connector_context.get("database_source"))
        .or_else(|| connector_context.get("databaseSource"))
        .filter(|value| value.is_object())
}

async fn fetch_mysql_source_connector(
    raw_config: &Value,
    raw_checkpoint: &Value,
    scope: ConnectorFetchScope,
) -> Result<Value> {
    let fetch_documents = matches!(
        scope,
        ConnectorFetchScope::Metadata | ConnectorFetchScope::Content
    );
    let include_body = matches!(scope, ConnectorFetchScope::Content);
    let config = MySqlSourceConfig::from_value(raw_config)
        .map_err(|error| anyhow!("invalid mysql_source connector config: {error}"))?;
    let checkpoint = MySqlSourceCheckpoint::from_value(raw_checkpoint)
        .map_err(|error| anyhow!("invalid mysql_source checkpoint: {error}"))?;
    let fetch_report = if fetch_documents {
        fetch_mysql_documents_report_with_checkpoint(&config, include_body, &checkpoint)
            .await
            .map_err(|error| anyhow!("mysql_source connector fetch failed: {error}"))?
    } else {
        Default::default()
    };
    let documents = fetch_report.documents;
    let next_checkpoint = if fetch_report
        .next_checkpoint
        .as_object()
        .map(|object| object.is_empty())
        .unwrap_or(true)
    {
        mysql_next_checkpoint_from_documents(&documents)
    } else {
        fetch_report.next_checkpoint
    };
    let table_counts = fetch_report
        .table_counts
        .iter()
        .map(|stats| {
            json!({
                "table": stats.table.clone(),
                "document_count": stats.document_count,
                "row_count": stats.row_count,
                "skipped_row_count": stats.skipped_row_count,
                "failed_row_count": stats.failed_row_count,
            })
        })
        .collect::<Vec<_>>();
    let row_failure_samples = fetch_report
        .row_failure_samples
        .iter()
        .map(|sample| {
            json!({
                "table": sample.table.clone(),
                "row_index": sample.row_index,
                "reason": sample.reason.clone(),
                "source_primary_key": sample.source_primary_key.clone(),
            })
        })
        .collect::<Vec<_>>();
    let tables = config
        .tables
        .iter()
        .map(|mapping| mapping.table.clone())
        .collect::<Vec<_>>();

    Ok(json!({
        "platform": "mysql",
        "users": [],
        "groups": [],
        "departments": [],
        "roles": [],
        "documents": documents,
        "acl_snapshots": [],
        "connector_summary": {
            "kind": "mysql_source",
            "database": config.database,
            "document_count": documents.len(),
            "row_count": fetch_report.row_count,
            "skipped_row_count": fetch_report.skipped_row_count,
            "failed_row_count": fetch_report.failed_row_count,
            "table_count": tables.len(),
            "tables": tables,
        },
        "mysql_fetch_summary": {
            "document_count": documents.len(),
            "row_count": fetch_report.row_count,
            "skipped_row_count": fetch_report.skipped_row_count,
            "failed_row_count": fetch_report.failed_row_count,
            "table_counts": table_counts,
            "row_failure_samples": row_failure_samples,
        },
        "next_checkpoint": next_checkpoint,
    }))
}

#[derive(Clone, Debug)]
struct HttpSourceConfig {
    base_url: String,
    allow_http_loopback: bool,
    platform: String,
    bearer_token: Option<String>,
    endpoints: Value,
    timeout_ms: u64,
    document_limit: usize,
    max_pages: usize,
    updated_after: Option<String>,
}

fn http_source_config_value(connector_context: &Value) -> Option<&Value> {
    connector_context
        .get("http_source")
        .or_else(|| connector_context.get("httpSource"))
        .or_else(|| connector_context.get("source_api"))
        .or_else(|| connector_context.get("sourceApi"))
        .filter(|value| value.is_object())
        .or_else(|| {
            if connector_context.get("base_url").is_some()
                || connector_context.get("baseUrl").is_some()
            {
                Some(connector_context)
            } else {
                None
            }
        })
}

fn fetch_http_source_connector(
    context: &Value,
    raw_config: &Value,
    scope: ConnectorFetchScope,
) -> Result<Value> {
    let config = HttpSourceConfig::from_value(context, raw_config)?;
    let client = Client::builder()
        .timeout(StdDuration::from_millis(config.timeout_ms))
        .build()?;
    let fetch_users = matches!(scope, ConnectorFetchScope::Users);
    let fetch_documents = matches!(
        scope,
        ConnectorFetchScope::Acl | ConnectorFetchScope::Metadata | ConnectorFetchScope::Content
    );
    let fetch_acl = matches!(
        scope,
        ConnectorFetchScope::Acl | ConnectorFetchScope::Content
    );
    let fetch_content = matches!(scope, ConnectorFetchScope::Content);
    let users = if fetch_users {
        http_get_items(&client, &config, "users", &[])?
    } else {
        Vec::new()
    };
    let groups = if fetch_users {
        http_get_optional_items(&client, &config, "groups")?
    } else {
        Vec::new()
    };
    let departments = if fetch_users {
        http_get_optional_items(&client, &config, "departments")?
    } else {
        Vec::new()
    };
    let roles = if fetch_users {
        http_get_optional_items(&client, &config, "roles")?
    } else {
        Vec::new()
    };
    let documents = if fetch_documents {
        http_get_documents(&client, &config)?
    } else {
        Vec::new()
    };
    let mut acl_snapshots = Vec::new();
    let mut documents_with_content = Vec::with_capacity(documents.len());

    for document in documents {
        let mut object = document.as_object().cloned().unwrap_or_default();
        normalize_document_revision_field(&mut object);
        let document_value = Value::Object(object.clone());
        let document_external_id = required_string(
            &document_value,
            &["document_external_id", "documentExternalId"],
        )?;
        let revision_external_id = string_field(
            &document_value,
            &["revision_external_id", "revisionExternalId"],
        );

        if fetch_acl {
            let acl = http_get_document_acl(
                &client,
                &config,
                &document_external_id,
                revision_external_id.as_deref(),
            )?;
            let acl_hash = string_field(&acl, &["acl_hash", "aclHash"]);
            object.insert("acl_snapshot".to_string(), acl.clone());
            object.insert("acl_hash".to_string(), json!(acl_hash));
            acl_snapshots.push(json!({
                "document_external_id": document_external_id,
                "revision_external_id": revision_external_id,
                "acl": acl,
                "acl_hash": acl_hash,
            }));
        }

        if fetch_content {
            let content = http_get_document_content(
                &client,
                &config,
                &document_external_id,
                revision_external_id.as_deref(),
            )?;
            merge_document_content(&mut object, &content);
        }

        documents_with_content.push(Value::Object(object));
    }

    Ok(json!({
        "platform": config.platform,
        "users": users,
        "groups": groups,
        "departments": departments,
        "roles": roles,
        "documents": documents_with_content,
        "acl_snapshots": acl_snapshots,
        "connector_summary": {
            "kind": "http_source",
            "base_url_redacted": redact_url(&config.base_url)?,
            "auth_mode": if config.bearer_token.is_some() { "bearer" } else { "none" },
            "document_count": documents_with_content.len(),
            "acl_snapshot_count": acl_snapshots.len(),
        }
    }))
}

impl HttpSourceConfig {
    fn from_value(context: &Value, value: &Value) -> Result<Self> {
        let base_url = string_field(value, &["base_url", "baseUrl"])
            .or_else(|| context_string(context, "base_url").ok().flatten())
            .ok_or_else(|| anyhow!("http_source connector requires base_url"))?;
        let allow_http_loopback = value
            .get("allow_http_loopback")
            .or_else(|| value.get("allowHttpLoopback"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        validate_http_source_base_url(&base_url, allow_http_loopback)?;
        let auth = value.get("auth").filter(|candidate| candidate.is_object());
        let bearer_token = if let Some(auth) = auth {
            resolve_bearer_token_from_auth(auth)?
        } else {
            None
        }
        .or_else(|| {
            value
                .get("bearer_token_env")
                .or_else(|| value.get("bearerTokenEnv"))
                .and_then(Value::as_str)
                .and_then(env_string)
        });
        let endpoints = value
            .get("endpoints")
            .filter(|candidate| candidate.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let checkpoint = context
            .get("checkpoint")
            .filter(|candidate| candidate.is_object());
        let updated_after = value
            .get("updated_after")
            .or_else(|| value.get("updatedAfter"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                checkpoint
                    .and_then(|checkpoint| {
                        checkpoint
                            .get("updated_after")
                            .or_else(|| checkpoint.get("updatedAfter"))
                            .and_then(Value::as_str)
                    })
                    .map(ToOwned::to_owned)
            });

        Ok(Self {
            base_url,
            allow_http_loopback,
            platform: string_field(value, &["platform"])
                .unwrap_or_else(|| "generic_chat".to_string()),
            bearer_token,
            endpoints,
            timeout_ms: numeric_field(value, &["timeout_ms", "timeoutMs"])
                .unwrap_or(DEFAULT_HTTP_TIMEOUT_MS),
            document_limit: numeric_field(value, &["document_limit", "documentLimit"])
                .and_then(|value| value.try_into().ok())
                .unwrap_or(DEFAULT_HTTP_DOCUMENT_LIMIT),
            max_pages: numeric_field(value, &["max_pages", "maxPages"])
                .and_then(|value| value.try_into().ok())
                .unwrap_or(DEFAULT_HTTP_MAX_PAGES),
            updated_after,
        })
    }
}

fn resolve_bearer_token_from_auth(auth: &Value) -> Result<Option<String>> {
    if string_field(auth, &["token", "bearer_token", "bearerToken"]).is_some() {
        return Err(anyhow!(
            "http_source connector_context must not contain raw bearer token; use token_env"
        ));
    }
    let token_env = string_field(
        auth,
        &[
            "token_env",
            "tokenEnv",
            "bearer_token_env",
            "bearerTokenEnv",
            "credential_env",
            "credentialEnv",
        ],
    );
    Ok(token_env.as_deref().and_then(env_string))
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn http_get_optional_items(
    client: &Client,
    config: &HttpSourceConfig,
    key: &str,
) -> Result<Vec<Value>> {
    if endpoint_template(&config.endpoints, key, "").is_empty() {
        return Ok(Vec::new());
    }
    http_get_items(client, config, key, &[])
}

fn http_get_items(
    client: &Client,
    config: &HttpSourceConfig,
    endpoint_key: &str,
    replacements: &[(&str, &str)],
) -> Result<Vec<Value>> {
    let body = http_get_json(client, config, endpoint_key, replacements)?;
    Ok(items_from_response(&body))
}

fn http_get_documents(client: &Client, config: &HttpSourceConfig) -> Result<Vec<Value>> {
    let mut cursor = String::new();
    let mut documents = Vec::new();
    for _ in 0..config.max_pages {
        let limit = config.document_limit.to_string();
        let updated_after = config.updated_after.as_deref().unwrap_or("");
        let body = http_get_json(
            client,
            config,
            "documents",
            &[
                ("cursor", cursor.as_str()),
                ("limit", limit.as_str()),
                ("iso_time", updated_after),
                ("updated_after", updated_after),
            ],
        )?;
        documents.extend(items_from_response(&body));
        let next_cursor = string_field(&body, &["next_cursor", "nextCursor"]).unwrap_or_default();
        if next_cursor.trim().is_empty() || next_cursor == cursor {
            break;
        }
        cursor = next_cursor;
    }
    Ok(documents)
}

fn http_get_document_content(
    client: &Client,
    config: &HttpSourceConfig,
    document_external_id: &str,
    revision_external_id: Option<&str>,
) -> Result<Value> {
    http_get_json(
        client,
        config,
        "document_content",
        &[
            ("document_external_id", document_external_id),
            ("revision", revision_external_id.unwrap_or("")),
            ("revision_external_id", revision_external_id.unwrap_or("")),
        ],
    )
}

fn http_get_document_acl(
    client: &Client,
    config: &HttpSourceConfig,
    document_external_id: &str,
    revision_external_id: Option<&str>,
) -> Result<Value> {
    http_get_json(
        client,
        config,
        "document_acl",
        &[
            ("document_external_id", document_external_id),
            ("revision", revision_external_id.unwrap_or("")),
            ("revision_external_id", revision_external_id.unwrap_or("")),
        ],
    )
}

fn http_get_json(
    client: &Client,
    config: &HttpSourceConfig,
    endpoint_key: &str,
    replacements: &[(&str, &str)],
) -> Result<Value> {
    let url = build_http_source_url(config, endpoint_key, replacements)?;
    let mut request = client.get(url);
    if let Some(token) = config.bearer_token.as_deref() {
        request = request.bearer_auth(token);
    }
    let response = request.send()?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!(
            "http_source endpoint {endpoint_key} returned status {}",
            status.as_u16()
        ));
    }
    Ok(response.json()?)
}

fn build_http_source_url(
    config: &HttpSourceConfig,
    endpoint_key: &str,
    replacements: &[(&str, &str)],
) -> Result<String> {
    let default_endpoint = match endpoint_key {
        "users" => "/users",
        "groups" => "",
        "departments" => "",
        "roles" => "",
        "documents" => "/documents?cursor={cursor}&updated_after={updated_after}&limit={limit}",
        "document_content" => "/documents/{document_external_id}/content?revision={revision}",
        "document_acl" => "/documents/{document_external_id}/acl?revision={revision}",
        _ => "",
    };
    let mut endpoint = endpoint_template(&config.endpoints, endpoint_key, default_endpoint);
    if endpoint.is_empty() {
        return Err(anyhow!(
            "http_source endpoint {endpoint_key} is not configured"
        ));
    }
    for (key, value) in replacements {
        endpoint = endpoint.replace(&format!("{{{key}}}"), &url_component_encode(value));
    }
    while let Some(start) = endpoint.find('{') {
        let Some(relative_end) = endpoint[start..].find('}') else {
            break;
        };
        endpoint.replace_range(start..=start + relative_end, "");
    }
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        validate_http_source_base_url(&endpoint, config.allow_http_loopback)?;
        return Ok(endpoint);
    }
    let base = config.base_url.trim_end_matches('/');
    let path = if endpoint.starts_with('/') {
        endpoint
    } else {
        format!("/{endpoint}")
    };
    Ok(format!("{base}{path}"))
}

fn endpoint_template(endpoints: &Value, key: &str, default_value: &str) -> String {
    endpoints
        .get(key)
        .or_else(|| endpoints.get(to_camel_case(key).as_str()))
        .and_then(Value::as_str)
        .map(strip_http_method)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_value.to_string())
}

fn strip_http_method(value: &str) -> String {
    let trimmed = value.trim();
    for method in ["GET ", "get "] {
        if let Some(path) = trimmed.strip_prefix(method) {
            return path.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn items_from_response(value: &Value) -> Vec<Value> {
    value
        .get("items")
        .or_else(|| value.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| value.as_array().cloned())
        .unwrap_or_default()
}

fn normalize_document_revision_field(object: &mut Map<String, Value>) {
    if !object.contains_key("revision_external_id") {
        if let Some(revision) = object
            .get("revision")
            .or_else(|| object.get("revisionExternalId"))
            .cloned()
        {
            object.insert("revision_external_id".to_string(), revision);
        }
    }
}

fn merge_document_content(document: &mut Map<String, Value>, content: &Value) {
    if let Some(content_object) = content.as_object() {
        for key in [
            "body",
            "content",
            "text",
            "title",
            "content_type",
            "contentType",
            "content_hash",
            "contentHash",
        ] {
            if let Some(value) = content_object.get(key) {
                document.insert(key.to_string(), value.clone());
            }
        }
        normalize_document_revision_field(document);
    }
}

fn validate_http_source_base_url(raw_url: &str, allow_http_loopback: bool) -> Result<()> {
    let url = reqwest::Url::parse(raw_url)?;
    if url.scheme() == "https" {
        return Ok(());
    }
    if url.scheme() == "http" && allow_http_loopback && is_loopback_host(url.host_str()) {
        return Ok(());
    }
    Err(anyhow!(
        "http_source base_url must use HTTPS unless loopback is explicitly allowed"
    ))
}

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("127.0.0.1" | "localhost" | "::1"))
}

fn redact_url(raw_url: &str) -> Result<String> {
    let url = reqwest::Url::parse(raw_url)?;
    Ok(format!(
        "{}://{}{}",
        url.scheme(),
        url.host_str().unwrap_or("[unknown]"),
        url.port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default()
    ))
}

fn url_component_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

fn to_camel_case(value: &str) -> String {
    let mut result = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if ch == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            result.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn numeric_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn connector_platform(connector: &Value) -> String {
    string_field(connector, &["platform"]).unwrap_or_else(|| "third_party".to_string())
}

fn external_document_metadata_from_connector(connector: &Value) -> Result<Vec<Value>> {
    let documents_value = connector
        .get("documents")
        .ok_or_else(|| anyhow!("external connector requires documents array"))?;
    let documents = value_array(Some(documents_value));

    documents
        .into_iter()
        .map(|document| {
            let document_external_id =
                required_string(&document, &["document_external_id", "documentExternalId"])?;
            let mut object = document.as_object().cloned().unwrap_or_default();
            normalize_document_revision_field(&mut object);
            object.insert(
                "document_external_id".to_string(),
                json!(document_external_id),
            );
            object
                .entry("title".to_string())
                .or_insert_with(|| json!("Untitled external document"));
            object.remove("body");
            object.remove("content");
            object.remove("text");
            Ok(Value::Object(object))
        })
        .collect()
}

fn external_documents_from_connector(connector: &Value) -> Result<Vec<Value>> {
    let documents_value = connector
        .get("documents")
        .ok_or_else(|| anyhow!("external connector requires documents array"))?;
    let documents = value_array(Some(documents_value));

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
                    &["revision_external_id", "revisionExternalId", "revision"]
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
    let revision_external_id = string_field(
        document,
        &["revision_external_id", "revisionExternalId", "revision"],
    );
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
    let revision_external_id = string_field(
        value,
        &["revision_external_id", "revisionExternalId", "revision"],
    );
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
        "row_count",
        "metadata_document_count",
        "metadata_row_count",
        "content_document_count",
        "content_row_count",
        "skipped_row_count",
        "failed_row_count",
    ] {
        if let Some(value) = output.get(key).and_then(Value::as_u64) {
            counts.insert(key.to_string(), json!(value));
        }
    }
    for key in ["metadata_table_counts", "content_table_counts"] {
        if let Some(value) = output.get(key).filter(|value| value.is_array()) {
            counts.insert(key.to_string(), value.clone());
        }
    }
    if let Some(value) = output
        .get("row_failure_samples")
        .filter(|value| value.is_array())
    {
        counts.insert("row_failure_samples".to_string(), value.clone());
    }
    let next_checkpoint = output
        .get("next_checkpoint")
        .filter(|value| {
            value
                .as_object()
                .map(|object| !object.is_empty())
                .unwrap_or(false)
        })
        .cloned()
        .unwrap_or_else(|| json!({}));
    if counts.is_empty()
        && next_checkpoint
            .as_object()
            .map(|object| object.is_empty())
            .unwrap_or(true)
    {
        return Ok(());
    }

    sqlx::query(
        r#"
        update external_sync_runs
        set counts = counts || $3,
            checkpoint = checkpoint || $4,
            updated_at = $5
        where tenant_id = $1 and id = $2
        "#,
    )
    .bind(tenant_id.0)
    .bind(sync_run_id)
    .bind(Value::Object(counts))
    .bind(next_checkpoint)
    .bind(Utc::now())
    .execute(storage.pool())
    .await?;

    Ok(())
}

async fn update_external_sync_failure_summary(
    storage: &PgStorage,
    tenant_id: TenantId,
    context: &Value,
    task_key: &str,
    error_message: &str,
) -> Result<()> {
    let Some(sync_run_id) = context
        .get("external_sync_run_id")
        .and_then(Value::as_str)
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
    else {
        return Ok(());
    };
    let error_excerpt = external_sync_error_excerpt(error_message);
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
    .bind(json!({
        "failed_task_key": task_key,
        "last_error": error_excerpt,
    }))
    .bind(Utc::now())
    .execute(storage.pool())
    .await?;
    Ok(())
}

fn external_sync_error_excerpt(error_message: &str) -> String {
    error_message
        .chars()
        .filter(|ch| !ch.is_control() || ch.is_whitespace())
        .collect::<String>()
        .trim()
        .chars()
        .take(240)
        .collect()
}

#[derive(Clone, Debug)]
struct ConnectorDatabaseFetchSummary {
    row_count: u64,
    skipped_row_count: u64,
    failed_row_count: u64,
    row_failure_samples: Value,
    table_counts: Value,
}

fn connector_database_fetch_summary(
    connector: &Value,
    documents: &[Value],
    count_key: &str,
) -> ConnectorDatabaseFetchSummary {
    let Some(summary) = connector
        .get("mysql_fetch_summary")
        .or_else(|| connector.get("database_fetch_summary"))
        .filter(|value| value.is_object())
    else {
        return ConnectorDatabaseFetchSummary {
            row_count: documents.len() as u64,
            skipped_row_count: 0,
            failed_row_count: 0,
            row_failure_samples: json!([]),
            table_counts: summarize_external_documents_by_table(documents, count_key),
        };
    };
    let table_counts = summary
        .get("table_counts")
        .and_then(Value::as_array)
        .map(|rows| {
            Value::Array(
                rows.iter()
                    .filter_map(|row| {
                        let table = string_field(row, &["table"])?;
                        let document_count = row
                            .get("document_count")
                            .or_else(|| row.get("documentCount"))
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        Some(json!({
                            "table": table,
                            count_key: document_count,
                            "row_count": row
                                .get("row_count")
                                .or_else(|| row.get("rowCount"))
                                .and_then(Value::as_u64)
                                .unwrap_or(document_count),
                            "skipped_row_count": row
                                .get("skipped_row_count")
                                .or_else(|| row.get("skippedRowCount"))
                                .and_then(Value::as_u64)
                                .unwrap_or(0),
                            "failed_row_count": row
                                .get("failed_row_count")
                                .or_else(|| row.get("failedRowCount"))
                                .and_then(Value::as_u64)
                                .unwrap_or(0),
                        }))
                    })
                    .collect(),
            )
        })
        .filter(|value| {
            value
                .as_array()
                .map(|rows| !rows.is_empty())
                .unwrap_or(false)
        })
        .unwrap_or_else(|| summarize_external_documents_by_table(documents, count_key));

    ConnectorDatabaseFetchSummary {
        row_count: summary
            .get("row_count")
            .or_else(|| summary.get("rowCount"))
            .and_then(Value::as_u64)
            .unwrap_or(documents.len() as u64),
        skipped_row_count: summary
            .get("skipped_row_count")
            .or_else(|| summary.get("skippedRowCount"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        failed_row_count: summary
            .get("failed_row_count")
            .or_else(|| summary.get("failedRowCount"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        row_failure_samples: normalize_database_row_failure_samples(
            summary
                .get("row_failure_samples")
                .or_else(|| summary.get("rowFailureSamples")),
        ),
        table_counts,
    }
}

fn normalize_database_row_failure_samples(value: Option<&Value>) -> Value {
    let rows = value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(8)
                .filter_map(|item| {
                    let table = string_field(item, &["table"])?;
                    Some(json!({
                        "table": table,
                        "row_index": item
                            .get("row_index")
                            .or_else(|| item.get("rowIndex"))
                            .and_then(Value::as_u64)
                            .unwrap_or(0),
                        "reason": string_field(item, &["reason"])
                            .unwrap_or_else(|| "row_conversion_failed".to_string())
                            .chars()
                            .filter(|ch| !ch.is_control() || ch.is_whitespace())
                            .take(160)
                            .collect::<String>(),
                        "source_primary_key": string_field(
                            item,
                            &["source_primary_key", "sourcePrimaryKey"],
                        )
                        .unwrap_or_default(),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Value::Array(rows)
}

fn summarize_external_documents_by_table(documents: &[Value], count_key: &str) -> Value {
    let mut counts = BTreeMap::<String, u64>::new();
    for document in documents {
        let table = external_document_source_table(document);
        *counts.entry(table).or_default() += 1;
    }
    Value::Array(
        counts
            .into_iter()
            .map(|(table, count)| {
                json!({
                    "table": table,
                    count_key: count,
                    "row_count": count,
                    "skipped_row_count": 0,
                    "failed_row_count": 0,
                })
            })
            .collect(),
    )
}

fn mysql_next_checkpoint_from_documents(documents: &[Value]) -> Value {
    let mut tables = BTreeMap::<String, Map<String, Value>>::new();
    for document in documents {
        let table = external_document_source_table(document);
        if table == "[unmapped]" {
            continue;
        }
        let Some(metadata) = document.get("metadata").filter(|value| value.is_object()) else {
            continue;
        };
        let entry = tables.entry(table).or_default();
        if let Some(updated_at) = string_field(metadata, &["source_updated_at", "sourceUpdatedAt"])
        {
            update_checkpoint_max_string(entry, "updated_after", updated_at);
        }
        if let Some(version) = string_field(metadata, &["source_version", "sourceVersion"]) {
            update_checkpoint_max_numeric_or_string(entry, "version_after", version);
        }
        let identity_columns = metadata
            .get("source_primary_key_columns")
            .or_else(|| metadata.get("sourcePrimaryKeyColumns"))
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(1);
        if identity_columns == 1 {
            if let Some(primary_key) =
                string_field(metadata, &["source_primary_key", "sourcePrimaryKey"])
            {
                update_checkpoint_max_numeric_or_string(entry, "last_id", primary_key);
            }
        }
    }
    if tables.is_empty() {
        return json!({});
    }
    json!({ "tables": tables })
}

fn update_checkpoint_max_string(entry: &mut Map<String, Value>, key: &str, candidate: String) {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return;
    }
    let should_replace = entry
        .get(key)
        .and_then(Value::as_str)
        .map(|current| candidate > current)
        .unwrap_or(true);
    if should_replace {
        entry.insert(key.to_string(), Value::String(candidate.to_string()));
    }
}

fn update_checkpoint_max_numeric_or_string(
    entry: &mut Map<String, Value>,
    key: &str,
    candidate: String,
) {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return;
    }
    let should_replace = entry
        .get(key)
        .and_then(Value::as_str)
        .map(
            |current| match (candidate.parse::<f64>(), current.parse::<f64>()) {
                (Ok(candidate), Ok(current)) => candidate > current,
                _ => candidate > current,
            },
        )
        .unwrap_or(true);
    if should_replace {
        entry.insert(key.to_string(), Value::String(candidate.to_string()));
    }
}

fn external_document_source_table(document: &Value) -> String {
    document
        .get("metadata")
        .filter(|value| value.is_object())
        .and_then(|metadata| {
            string_field(
                metadata,
                &["source_table", "sourceTable", "table", "source_table_name"],
            )
        })
        .or_else(|| string_field(document, &["source_table", "sourceTable", "table"]))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "[unmapped]".to_string())
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
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
    };

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
                            "path": "/risk/doc-001",
                            "source_table": "risk_policy"
                        }
                    }]
                }
            }
        })
    }

    #[tokio::test]
    async fn connector_fixture_accepts_mock_https_connector_shape() {
        let connector = connector_fixture(&sample_context())
            .await
            .expect("connector fixture");

        assert_eq!(connector_platform(&connector), "generic_chat");
        assert_eq!(value_array(connector.get("users")).len(), 1);
        assert_eq!(value_array(connector.get("documents")).len(), 1);
    }

    #[tokio::test]
    async fn acl_snapshots_can_be_built_from_direct_acl_fields() {
        let connector = connector_fixture(&sample_context())
            .await
            .expect("connector fixture");
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

    #[tokio::test]
    async fn fetch_content_output_includes_body_and_matching_acl_snapshot() {
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

        let output = build_fetch_external_content_output(&execution)
            .await
            .expect("content output");

        assert_eq!(output["document_count"], json!(1));
        assert_eq!(output["row_count"], json!(1));
        assert_eq!(output["skipped_row_count"], json!(0));
        assert_eq!(output["failed_row_count"], json!(0));
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
        assert_eq!(
            output["content_table_counts"],
            json!([{
                "table": "risk_policy",
                "content_document_count": 1,
                "row_count": 1,
                "skipped_row_count": 0,
                "failed_row_count": 0
            }])
        );
    }

    #[tokio::test]
    async fn metadata_output_redacts_body_before_workflow_event() {
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

        let output = build_sync_external_metadata_output(&execution)
            .await
            .expect("metadata output");

        assert_eq!(output["document_count"], json!(1));
        assert_eq!(output["row_count"], json!(1));
        assert_eq!(output["skipped_row_count"], json!(0));
        assert_eq!(output["failed_row_count"], json!(0));
        assert!(output["external_document_metadata"][0]
            .get("body")
            .is_none());
        assert!(output["external_document_metadata"][0]
            .get("content")
            .is_none());
        assert!(output["external_document_metadata"][0]
            .get("text")
            .is_none());
        assert_eq!(
            output["metadata_table_counts"],
            json!([{
                "table": "risk_policy",
                "metadata_document_count": 1,
                "row_count": 1,
                "skipped_row_count": 0,
                "failed_row_count": 0
            }])
        );
    }

    #[test]
    fn mysql_next_checkpoint_summarizes_table_high_water_marks() {
        let checkpoint = mysql_next_checkpoint_from_documents(&[
            json!({
                "metadata": {
                    "source_table": "bi_traffic_area",
                    "source_primary_key": "9",
                    "source_primary_key_columns": ["id"],
                    "source_updated_at": "2026-05-20 10:00:00",
                    "source_version": "12"
                }
            }),
            json!({
                "metadata": {
                    "source_table": "bi_traffic_area",
                    "source_primary_key": "10",
                    "source_primary_key_columns": ["id"],
                    "source_updated_at": "2026-05-21 09:00:00",
                    "source_version": "13"
                }
            }),
            json!({
                "metadata": {
                    "source_table": "composite_table",
                    "source_primary_key": "tenant=1|id=10",
                    "source_primary_key_columns": ["tenant", "id"],
                    "source_updated_at": "2026-05-19 08:00:00"
                }
            }),
        ]);

        assert_eq!(
            checkpoint,
            json!({
                "tables": {
                    "bi_traffic_area": {
                        "last_id": "10",
                        "updated_after": "2026-05-21 09:00:00",
                        "version_after": "13"
                    },
                    "composite_table": {
                        "updated_after": "2026-05-19 08:00:00"
                    }
                }
            })
        );
    }

    #[test]
    fn connector_database_fetch_summary_keeps_failed_mysql_rows_visible() {
        let documents = vec![json!({
            "metadata": {
                "source_table": "bi_traffic_area"
            }
        })];
        let connector = json!({
            "mysql_fetch_summary": {
                "document_count": 1,
                "row_count": 3,
                "skipped_row_count": 0,
                "failed_row_count": 2,
                "row_failure_samples": [{
                    "table": "bi_traffic_area",
                    "row_index": 3,
                    "source_primary_key": "42",
                    "reason": "mapped row has empty identity columns"
                }],
                "table_counts": [{
                    "table": "bi_traffic_area",
                    "document_count": 1,
                    "row_count": 3,
                    "skipped_row_count": 0,
                    "failed_row_count": 2
                }]
            }
        });

        let summary =
            connector_database_fetch_summary(&connector, &documents, "content_document_count");

        assert_eq!(summary.row_count, 3);
        assert_eq!(summary.failed_row_count, 2);
        assert_eq!(
            summary.row_failure_samples,
            json!([{
                "table": "bi_traffic_area",
                "row_index": 3,
                "reason": "mapped row has empty identity columns",
                "source_primary_key": "42"
            }])
        );
        assert_eq!(
            summary.table_counts,
            json!([{
                "table": "bi_traffic_area",
                "content_document_count": 1,
                "row_count": 3,
                "skipped_row_count": 0,
                "failed_row_count": 2
            }])
        );
    }

    #[test]
    fn external_document_metadata_allows_empty_documents_array() {
        let metadata = external_document_metadata_from_connector(&json!({
            "platform": "mysql",
            "documents": []
        }))
        .expect("empty metadata should be valid");

        assert!(metadata.is_empty());
    }

    #[tokio::test]
    async fn mysql_source_users_scope_returns_empty_principals_without_live_connection() {
        let context = json!({
            "connector_context": {
                "mysql_source": {
                    "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
                    "database": "hy_sql",
                    "tables": [{
                        "table": "bi_traffic_area",
                        "id_column": "id",
                        "content_columns": ["name"]
                    }]
                }
            }
        });

        let connector = connector_fixture_for(&context, ConnectorFetchScope::Users)
            .await
            .expect("users scope does not connect to mysql");

        assert_eq!(connector["platform"], json!("mysql"));
        assert!(value_array(connector.get("users")).is_empty());
        assert!(value_array(connector.get("documents")).is_empty());
        assert_eq!(
            connector["connector_summary"]["tables"],
            json!(["bi_traffic_area"])
        );
    }

    #[tokio::test]
    async fn http_source_connector_fetches_users_documents_content_and_acl() {
        let (base_url, stop, handle) = start_http_source_fixture_server();
        std::env::set_var("AIV3_TEST_HTTP_SOURCE_TOKEN", "test-source-token");
        let context = json!({
            "source_id": "src-http",
            "external_sync_run_id": uuid::Uuid::new_v4().to_string(),
            "connector_context": {
                "http_source": {
                    "platform": "generic_chat",
                    "base_url": base_url,
                    "allow_http_loopback": true,
                    "auth": {
                        "request_auth_modes": ["bearer"],
                        "token_env": "AIV3_TEST_HTTP_SOURCE_TOKEN"
                    },
                    "endpoints": {
                        "users": "GET /users",
                        "documents": "GET /documents?limit={limit}&cursor={cursor}",
                        "document_content": "GET /documents/{document_external_id}/content?revision={revision}",
                        "document_acl": "GET /documents/{document_external_id}/acl?revision={revision}"
                    }
                }
            }
        });

        let users_connector = connector_fixture_for(&context, ConnectorFetchScope::Users)
            .await
            .expect("HTTP source users connector");
        let connector = connector_fixture_for(&context, ConnectorFetchScope::Content)
            .await
            .expect("HTTP source content connector");
        stop.store(true, Ordering::Relaxed);
        handle.join().expect("fixture server should stop");
        std::env::remove_var("AIV3_TEST_HTTP_SOURCE_TOKEN");

        assert_eq!(connector_platform(&users_connector), "generic_chat");
        assert_eq!(value_array(users_connector.get("users")).len(), 1);
        assert_eq!(
            users_connector["users"][0]["group_external_ids"],
            json!(["group-procurement"])
        );
        assert_eq!(value_array(connector.get("documents")).len(), 1);
        assert_eq!(
            connector["documents"][0]["revision_external_id"],
            json!("rev-http-1")
        );
        assert_eq!(
            connector["documents"][0]["body"],
            json!("HTTP source content body.")
        );
        assert_eq!(value_array(connector.get("acl_snapshots")).len(), 1);
        assert_eq!(
            connector["acl_snapshots"][0]["acl"]["allow"][0]["subject_external_id"],
            json!("group-procurement")
        );
    }

    #[tokio::test]
    async fn http_source_metadata_scope_fetches_document_list_without_content_endpoint() {
        let (base_url, stop, handle) = start_http_source_fixture_server();
        std::env::set_var("AIV3_TEST_HTTP_SOURCE_TOKEN", "test-source-token");
        let context = json!({
            "source_id": "src-http",
            "connector_context": {
                "http_source": {
                    "base_url": base_url,
                    "allow_http_loopback": true,
                    "auth": {
                        "token_env": "AIV3_TEST_HTTP_SOURCE_TOKEN"
                    },
                    "endpoints": {
                        "documents": "GET /documents?limit={limit}"
                    }
                }
            }
        });

        let connector = connector_fixture_for(&context, ConnectorFetchScope::Metadata)
            .await
            .expect("HTTP source metadata connector");
        let metadata =
            external_document_metadata_from_connector(&connector).expect("metadata documents");
        stop.store(true, Ordering::Relaxed);
        handle.join().expect("fixture server should stop");
        std::env::remove_var("AIV3_TEST_HTTP_SOURCE_TOKEN");

        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0]["document_external_id"], json!("doc-http"));
        assert!(metadata[0].get("body").is_none());
    }

    #[tokio::test]
    async fn http_source_connector_rejects_raw_bearer_token_in_context() {
        let context = json!({
            "connector_context": {
                "http_source": {
                    "base_url": "http://127.0.0.1:1",
                    "allow_http_loopback": true,
                    "auth": {
                        "token": "test-source-token"
                    }
                }
            }
        });

        let error = connector_fixture(&context)
            .await
            .expect_err("raw token should be rejected");

        assert!(error
            .to_string()
            .contains("must not contain raw bearer token"));
    }

    fn start_http_source_fixture_server() -> (String, Arc<AtomicBool>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
        listener
            .set_nonblocking(true)
            .expect("set fixture listener nonblocking");
        let address = listener.local_addr().expect("fixture server address");
        let stop = Arc::new(AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !server_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => handle_http_source_fixture_client(stream),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        (format!("http://{address}"), stop, handle)
    }

    fn handle_http_source_fixture_client(mut stream: TcpStream) {
        let mut buffer = [0_u8; 4096];
        let read = stream.read(&mut buffer).unwrap_or(0);
        let request = String::from_utf8_lossy(&buffer[..read]);
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");
        if !request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-source-token")
        {
            write_fixture_response(&mut stream, 401, r#"{"error":"source_auth_failed"}"#);
            return;
        }
        let body = if path.starts_with("/users") {
            r#"{"items":[{"external_user_id":"user-http","status":"active","group_external_ids":["group-procurement"]}],"next_cursor":null}"#
        } else if path.starts_with("/documents/doc-http/content") {
            r#"{"document_external_id":"doc-http","revision":"rev-http-1","content_type":"text/markdown","body":"HTTP source content body."}"#
        } else if path.starts_with("/documents/doc-http/acl") {
            r#"{"document_external_id":"doc-http","revision":"rev-http-1","acl_hash":"acl-http-1","allow":[{"subject_type":"group","subject_external_id":"group-procurement","level":"read"}],"deny":[]}"#
        } else if path.starts_with("/documents") {
            r#"{"items":[{"document_external_id":"doc-http","title":"HTTP Source Doc","revision":"rev-http-1","content_type":"text/markdown"}],"next_cursor":null}"#
        } else {
            r#"{"error":"not_found"}"#
        };
        let status = if body.contains("not_found") { 404 } else { 200 };
        write_fixture_response(&mut stream, status, body);
    }

    fn write_fixture_response(stream: &mut TcpStream, status: u16, body: &str) {
        let reason = if status == 200 { "OK" } else { "Error" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    }
}
