use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{mysql::MySqlPoolOptions, MySqlPool, Row};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_ROW_LIMIT: u32 = 1_000;
const MAX_TIMEOUT_MS: u64 = 120_000;
const MAX_ROW_LIMIT: u32 = 50_000;
const DEFAULT_CONTENT_TYPE: &str = "text/markdown";
const DEFAULT_OBJECT_TYPE: &str = "document";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DatabaseSourceError {
    #[error("{field} is required")]
    MissingRequiredField { field: &'static str },
    #[error("{field} is invalid: {reason}")]
    InvalidField { field: &'static str, reason: String },
    #[error("raw database secret field `{field}` is not accepted")]
    RawSecretField { field: String },
    #[error("invalid mysql identifier `{identifier}` in {field}")]
    InvalidIdentifier {
        field: &'static str,
        identifier: String,
    },
    #[error("invalid mysql source config: {0}")]
    InvalidConfig(String),
    #[error("mysql source environment variable `{env}` is not set")]
    MissingConnectionEnv { env: String },
    #[error("mysql source query failed: {0}")]
    Query(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MySqlSourceConfig {
    pub connection_env: String,
    pub database: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_row_limit")]
    pub row_limit: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_dataset_id: Option<String>,
    #[serde(default)]
    pub tables: Vec<MySqlTableMapping>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MySqlTableMapping {
    pub table: String,
    #[serde(default = "default_object_type")]
    pub object_type: String,
    pub id_column: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_column: Option<String>,
    #[serde(default)]
    pub content_columns: Vec<String>,
    #[serde(default = "default_content_type")]
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_column: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_column: Option<String>,
    #[serde(default)]
    pub revision_strategy: MySqlRevisionStrategy,
    #[serde(default)]
    pub metadata_columns: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MySqlRevisionStrategy {
    UpdatedAtHash,
    VersionColumn,
    #[default]
    ContentHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseSourceRedactedSummary {
    pub kind: String,
    pub database: String,
    pub connection_env: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_dataset_id: Option<String>,
    pub table_count: usize,
    pub tables: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseConnectionHealth {
    pub kind: String,
    pub status: String,
    pub database: String,
    pub server_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseSchemaSnapshot {
    pub kind: String,
    pub database: String,
    pub server_version: String,
    pub tables: Vec<DatabaseTableView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseTableView {
    pub name: String,
    pub table_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate_row_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<String>,
    pub columns: Vec<DatabaseColumnView>,
    pub primary_key_columns: Vec<String>,
    pub indexes: Vec<DatabaseIndexView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseColumnView {
    pub name: String,
    pub ordinal_position: u32,
    pub data_type: String,
    pub column_type: String,
    pub is_nullable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseIndexView {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TablePreview {
    pub table: String,
    pub row_limit: u32,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MySqlAggregateRequest {
    pub table: String,
    #[serde(default)]
    pub dimensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    #[serde(default = "default_aggregate")]
    pub aggregation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseAggregateResult {
    pub table: String,
    pub dimensions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    pub aggregation: String,
    pub row_limit: u32,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseSemanticProfile {
    pub kind: String,
    pub database: String,
    pub tables: Vec<DatabaseTableSemanticProfile>,
    pub report_suggestions: Vec<DatabaseReportSuggestion>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseTableSemanticProfile {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate_row_count: Option<u64>,
    pub column_count: usize,
    pub primary_key_columns: Vec<String>,
    pub columns: Vec<DatabaseColumnSemanticProfile>,
    pub dimensions: Vec<String>,
    pub metrics: Vec<String>,
    pub time_dimensions: Vec<String>,
    pub entity_columns: Vec<String>,
    pub text_columns: Vec<String>,
    pub suggested_mapping: DatabaseTableMappingSuggestion,
    pub suggested_questions: Vec<String>,
    pub suggested_visualizations: Vec<DatabaseVisualizationSuggestion>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseColumnSemanticProfile {
    pub name: String,
    pub data_type: String,
    pub column_type: String,
    pub semantic_role: String,
    pub role_confidence: u8,
    pub nullable: bool,
    pub sample_values: Vec<String>,
    pub distinct_sample_count: usize,
    pub null_sample_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseVisualizationSuggestion {
    pub title: String,
    pub chart_type: String,
    pub table: String,
    pub dimensions: Vec<String>,
    pub metrics: Vec<String>,
    pub rationale: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseReportSuggestion {
    pub title: String,
    pub objective: String,
    pub table: String,
    pub visualization: String,
    pub dimensions: Vec<String>,
    pub metrics: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseTableMappingSuggestion {
    pub table: String,
    pub object_type: String,
    pub id_column: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_column: Option<String>,
    pub content_columns: Vec<String>,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at_column: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_column: Option<String>,
    pub metadata_columns: Vec<String>,
    pub confidence: u8,
    pub rationale: String,
}

impl MySqlSourceConfig {
    pub fn from_value(value: &Value) -> Result<Self, DatabaseSourceError> {
        let normalized = unwrap_mysql_source_value(value);
        reject_raw_secret_fields(normalized)?;
        let mut config: Self = serde_json::from_value(normalized.clone())
            .map_err(|error| DatabaseSourceError::InvalidConfig(error.to_string()))?;
        config.normalize();
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), DatabaseSourceError> {
        validate_non_empty("connection_env", &self.connection_env)?;
        if !is_valid_env_name(&self.connection_env) {
            return Err(DatabaseSourceError::InvalidField {
                field: "connection_env",
                reason: "must be an environment variable name".to_string(),
            });
        }
        validate_non_empty("database", &self.database)?;
        validate_identifier("database", &self.database)?;

        if self.timeout_ms == 0 || self.timeout_ms > MAX_TIMEOUT_MS {
            return Err(DatabaseSourceError::InvalidField {
                field: "timeout_ms",
                reason: format!("must be between 1 and {MAX_TIMEOUT_MS}"),
            });
        }
        if self.row_limit == 0 || self.row_limit > MAX_ROW_LIMIT {
            return Err(DatabaseSourceError::InvalidField {
                field: "row_limit",
                reason: format!("must be between 1 and {MAX_ROW_LIMIT}"),
            });
        }
        if let Some(dataset_id) = self.default_dataset_id.as_deref() {
            validate_non_empty("default_dataset_id", dataset_id)?;
            Uuid::parse_str(dataset_id).map_err(|_| DatabaseSourceError::InvalidField {
                field: "default_dataset_id",
                reason: "must be a UUID dataset id".to_string(),
            })?;
        }

        for mapping in &self.tables {
            mapping.validate()?;
        }
        Ok(())
    }

    pub fn redacted_summary(&self) -> DatabaseSourceRedactedSummary {
        DatabaseSourceRedactedSummary {
            kind: "mysql".to_string(),
            database: self.database.clone(),
            connection_env: self.connection_env.clone(),
            default_dataset_id: self.default_dataset_id.clone(),
            table_count: self.tables.len(),
            tables: self
                .tables
                .iter()
                .map(|mapping| mapping.table.clone())
                .collect(),
        }
    }

    fn normalize(&mut self) {
        self.connection_env = self.connection_env.trim().to_string();
        self.database = self.database.trim().to_string();
        self.default_dataset_id = self
            .default_dataset_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        for mapping in &mut self.tables {
            mapping.normalize();
        }
    }
}

pub async fn test_mysql_connection(
    config: &MySqlSourceConfig,
) -> Result<DatabaseConnectionHealth, DatabaseSourceError> {
    config.validate()?;
    let pool = connect_mysql_pool(config).await?;
    let row = sqlx::query("select version() as server_version, database() as database_name")
        .fetch_one(&pool)
        .await
        .map_err(sqlx_error)?;
    Ok(DatabaseConnectionHealth {
        kind: "mysql".to_string(),
        status: "ok".to_string(),
        database: row
            .try_get::<Option<String>, _>("database_name")
            .map_err(sqlx_error)?
            .unwrap_or_else(|| config.database.clone()),
        server_version: row
            .try_get::<String, _>("server_version")
            .map_err(sqlx_error)?,
    })
}

pub async fn inspect_mysql_schema(
    config: &MySqlSourceConfig,
) -> Result<DatabaseSchemaSnapshot, DatabaseSourceError> {
    config.validate()?;
    let pool = connect_mysql_pool(config).await?;
    let health = test_mysql_connection(config).await?;

    let table_rows = sqlx::query(mysql_schema_tables_query())
        .bind(&config.database)
        .fetch_all(&pool)
        .await
        .map_err(sqlx_error)?;
    let column_rows = sqlx::query(mysql_schema_columns_query())
        .bind(&config.database)
        .fetch_all(&pool)
        .await
        .map_err(sqlx_error)?;
    let statistics_rows = sqlx::query(mysql_schema_statistics_query())
        .bind(&config.database)
        .fetch_all(&pool)
        .await
        .map_err(sqlx_error)?;

    let mut columns_by_table: BTreeMap<String, Vec<DatabaseColumnView>> = BTreeMap::new();
    for row in column_rows {
        let table_name: String = row.try_get("table_name").map_err(sqlx_error)?;
        columns_by_table
            .entry(table_name)
            .or_default()
            .push(DatabaseColumnView {
                name: row.try_get("column_name").map_err(sqlx_error)?,
                ordinal_position: row
                    .try_get::<u32, _>("ordinal_position")
                    .map_err(sqlx_error)?,
                data_type: row.try_get("data_type").map_err(sqlx_error)?,
                column_type: row.try_get("column_type").map_err(sqlx_error)?,
                is_nullable: row
                    .try_get::<String, _>("is_nullable")
                    .map_err(sqlx_error)?
                    .eq_ignore_ascii_case("YES"),
                default_value: row.try_get("column_default").map_err(sqlx_error)?,
                comment: empty_string_to_none(row.try_get("column_comment").map_err(sqlx_error)?),
            });
    }

    let indexes_by_table = index_views_by_table(statistics_rows)?;
    let mut tables = Vec::new();
    for row in table_rows {
        let name: String = row.try_get("table_name").map_err(sqlx_error)?;
        let indexes = indexes_by_table.get(&name).cloned().unwrap_or_default();
        let primary_key_columns = indexes
            .iter()
            .find(|index| index.name == "PRIMARY")
            .map(|index| index.columns.clone())
            .unwrap_or_default();
        tables.push(DatabaseTableView {
            name: name.clone(),
            table_type: row.try_get("table_type").map_err(sqlx_error)?,
            comment: empty_string_to_none(row.try_get("table_comment").map_err(sqlx_error)?),
            approximate_row_count: row
                .try_get::<Option<i64>, _>("table_rows")
                .map_err(sqlx_error)?
                .and_then(|value| u64::try_from(value).ok()),
            update_time: row.try_get("update_time").map_err(sqlx_error)?,
            columns: columns_by_table.remove(&name).unwrap_or_default(),
            primary_key_columns,
            indexes,
        });
    }

    Ok(DatabaseSchemaSnapshot {
        kind: "mysql".to_string(),
        database: health.database,
        server_version: health.server_version,
        tables,
    })
}

pub async fn preview_mysql_table(
    config: &MySqlSourceConfig,
    table: &str,
    limit: u32,
) -> Result<TablePreview, DatabaseSourceError> {
    config.validate()?;
    let plan = build_mysql_table_preview_query(config, table, limit)?;
    let pool = connect_mysql_pool(config).await?;
    let mut connection = pool.acquire().await.map_err(sqlx_error)?;
    sqlx::query("start transaction read only")
        .execute(&mut *connection)
        .await
        .map_err(sqlx_error)?;
    let query_result = sqlx::query(&plan.sql).fetch_all(&mut *connection).await;
    match query_result {
        Ok(rows) => {
            sqlx::query("commit")
                .execute(&mut *connection)
                .await
                .map_err(sqlx_error)?;
            build_table_preview(plan, rows)
        }
        Err(error) => {
            let _ = sqlx::query("rollback").execute(&mut *connection).await;
            Err(sqlx_error(error))
        }
    }
}

pub async fn aggregate_mysql_table(
    config: &MySqlSourceConfig,
    request: &MySqlAggregateRequest,
) -> Result<DatabaseAggregateResult, DatabaseSourceError> {
    config.validate()?;
    let plan = build_mysql_aggregate_query(config, request)?;
    let pool = connect_mysql_pool(config).await?;
    let rows = sqlx::query(&plan.sql)
        .fetch_all(&pool)
        .await
        .map_err(sqlx_error)?;
    build_aggregate_result(plan, rows)
}

pub async fn fetch_mysql_documents(
    config: &MySqlSourceConfig,
    include_body: bool,
) -> Result<Vec<Value>, DatabaseSourceError> {
    config.validate()?;
    let pool = connect_mysql_pool(config).await?;
    let mut documents = Vec::new();
    for mapping in &config.tables {
        let plan = build_mysql_document_fetch_query(config, mapping)?;
        let rows = sqlx::query(&plan.sql)
            .fetch_all(&pool)
            .await
            .map_err(sqlx_error)?;
        for row in rows {
            documents.push(mysql_row_to_external_document(
                mapping,
                &plan.columns,
                row,
                include_body,
            )?);
        }
    }
    Ok(documents)
}

pub async fn profile_mysql_database(
    config: &MySqlSourceConfig,
    sample_limit: u32,
) -> Result<DatabaseSemanticProfile, DatabaseSourceError> {
    let schema = inspect_mysql_schema(config).await?;
    let mut previews = Vec::new();
    for mapping in &config.tables {
        previews.push(preview_mysql_table(config, &mapping.table, sample_limit).await?);
    }
    Ok(build_database_semantic_profile(schema, previews))
}

pub fn build_database_semantic_profile(
    schema: DatabaseSchemaSnapshot,
    previews: Vec<TablePreview>,
) -> DatabaseSemanticProfile {
    let previews_by_table = previews
        .into_iter()
        .map(|preview| (preview.table.clone(), preview))
        .collect::<BTreeMap<_, _>>();
    let tables = schema
        .tables
        .into_iter()
        .map(|table| {
            let table_name = table.name.clone();
            build_table_semantic_profile(table, previews_by_table.get(&table_name))
        })
        .collect::<Vec<_>>();
    let report_suggestions = tables
        .iter()
        .flat_map(table_report_suggestions)
        .collect::<Vec<_>>();

    DatabaseSemanticProfile {
        kind: schema.kind,
        database: schema.database,
        tables,
        report_suggestions,
    }
}

fn build_table_semantic_profile(
    table: DatabaseTableView,
    preview: Option<&TablePreview>,
) -> DatabaseTableSemanticProfile {
    let columns = table
        .columns
        .iter()
        .map(|column| build_column_semantic_profile(column, &table.primary_key_columns, preview))
        .collect::<Vec<_>>();
    let dimensions = columns_by_role(&columns, &["dimension", "entity", "boolean"]);
    let metrics = columns_by_role(&columns, &["metric"]);
    let time_dimensions = columns_by_role(&columns, &["time"]);
    let entity_columns = columns_by_role(&columns, &["entity", "primary_key"]);
    let text_columns = columns_by_role(&columns, &["text"]);
    let suggested_mapping = suggested_table_mapping(
        &table.name,
        &table.primary_key_columns,
        &columns,
        &dimensions,
        &metrics,
        &time_dimensions,
        &entity_columns,
        &text_columns,
    );
    let suggested_visualizations =
        table_visualization_suggestions(&table.name, &dimensions, &metrics, &time_dimensions);
    let suggested_questions =
        table_suggested_questions(&table.name, &dimensions, &metrics, &time_dimensions);

    DatabaseTableSemanticProfile {
        name: table.name,
        approximate_row_count: table.approximate_row_count,
        column_count: columns.len(),
        primary_key_columns: table.primary_key_columns,
        columns,
        dimensions,
        metrics,
        time_dimensions,
        entity_columns,
        text_columns,
        suggested_mapping,
        suggested_questions,
        suggested_visualizations,
    }
}

fn build_column_semantic_profile(
    column: &DatabaseColumnView,
    primary_key_columns: &[String],
    preview: Option<&TablePreview>,
) -> DatabaseColumnSemanticProfile {
    let sample = column_sample(preview, &column.name);
    let (semantic_role, role_confidence) =
        classify_column_semantic_role(column, primary_key_columns, &sample);
    DatabaseColumnSemanticProfile {
        name: column.name.clone(),
        data_type: column.data_type.clone(),
        column_type: column.column_type.clone(),
        semantic_role: semantic_role.to_string(),
        role_confidence,
        nullable: column.is_nullable,
        sample_values: sample.sample_values,
        distinct_sample_count: sample.distinct_count,
        null_sample_count: sample.null_count,
    }
}

#[derive(Clone, Debug, Default)]
struct ColumnSample {
    sample_values: Vec<String>,
    distinct_count: usize,
    null_count: usize,
    non_null_count: usize,
    average_len: usize,
}

fn column_sample(preview: Option<&TablePreview>, column_name: &str) -> ColumnSample {
    let Some(preview) = preview else {
        return ColumnSample::default();
    };
    let mut distinct = BTreeSet::new();
    let mut sample_values = Vec::new();
    let mut null_count = 0usize;
    let mut non_null_count = 0usize;
    let mut total_len = 0usize;
    for row in &preview.rows {
        let Some(value) = row.as_object().and_then(|object| object.get(column_name)) else {
            null_count += 1;
            continue;
        };
        let normalized = match value {
            Value::Null => None,
            Value::String(value) => Some(value.trim().to_string()),
            other => Some(other.to_string()),
        }
        .filter(|value| !value.is_empty());
        match normalized {
            Some(value) => {
                non_null_count += 1;
                total_len += value.chars().count();
                distinct.insert(value.clone());
                if sample_values.len() < 5 && !sample_values.iter().any(|item| item == &value) {
                    sample_values.push(value);
                }
            }
            None => null_count += 1,
        }
    }
    ColumnSample {
        sample_values,
        distinct_count: distinct.len(),
        null_count,
        non_null_count,
        average_len: if non_null_count == 0 {
            0
        } else {
            total_len / non_null_count
        },
    }
}

fn classify_column_semantic_role(
    column: &DatabaseColumnView,
    primary_key_columns: &[String],
    sample: &ColumnSample,
) -> (&'static str, u8) {
    let name = column.name.to_ascii_lowercase();
    let data_type = column.data_type.to_ascii_lowercase();
    let column_type = column.column_type.to_ascii_lowercase();
    if primary_key_columns
        .iter()
        .any(|candidate| candidate == &column.name)
    {
        return ("primary_key", 100);
    }
    if is_boolean_type(&data_type, &column_type, &name) {
        return ("boolean", 90);
    }
    if is_time_type(&data_type) || name_has_any(&name, &["date", "time", "year", "month", "day"]) {
        return ("time", 90);
    }
    if is_numeric_type(&data_type) {
        if looks_like_identifier(&name) {
            return ("entity", 85);
        }
        if looks_like_metric(&name) {
            return ("metric", 90);
        }
        if sample.non_null_count > 0 && sample.distinct_count <= 20 {
            return ("dimension", 72);
        }
        return ("metric", 78);
    }
    if is_text_type(&data_type) {
        if looks_like_entity(&name) {
            return ("entity", 82);
        }
        if looks_like_dimension(&name)
            || (sample.non_null_count > 0
                && sample.distinct_count <= 50
                && sample.average_len <= 60)
        {
            return ("dimension", 78);
        }
        if sample.average_len > 80
            || name_has_any(
                &name,
                &["content", "desc", "detail", "remark", "memo", "note"],
            )
        {
            return ("text", 76);
        }
        return ("dimension", 65);
    }
    ("unknown", 40)
}

fn columns_by_role(columns: &[DatabaseColumnSemanticProfile], roles: &[&str]) -> Vec<String> {
    columns
        .iter()
        .filter(|column| roles.iter().any(|role| *role == column.semantic_role))
        .map(|column| column.name.clone())
        .collect()
}

fn suggested_table_mapping(
    table: &str,
    primary_key_columns: &[String],
    columns: &[DatabaseColumnSemanticProfile],
    dimensions: &[String],
    metrics: &[String],
    time_dimensions: &[String],
    entity_columns: &[String],
    text_columns: &[String],
) -> DatabaseTableMappingSuggestion {
    let id_column = primary_key_columns
        .first()
        .cloned()
        .or_else(|| {
            columns
                .iter()
                .find(|column| looks_like_identifier(&column.name.to_ascii_lowercase()))
                .map(|column| column.name.clone())
        })
        .or_else(|| entity_columns.first().cloned())
        .or_else(|| columns.first().map(|column| column.name.clone()))
        .unwrap_or_else(|| "id".to_string());
    let title_column = entity_columns
        .iter()
        .chain(dimensions.iter())
        .chain(text_columns.iter())
        .find(|column| *column != &id_column)
        .cloned();
    let updated_at_column = time_dimensions
        .iter()
        .find(|column| {
            let normalized = column.to_ascii_lowercase();
            name_has_any(
                &normalized,
                &["updated", "modified", "changed", "time", "date"],
            )
        })
        .cloned()
        .or_else(|| time_dimensions.first().cloned());
    let version_column = columns
        .iter()
        .find(|column| {
            let normalized = column.name.to_ascii_lowercase();
            name_has_any(&normalized, &["version", "revision", "rev"])
        })
        .map(|column| column.name.clone());

    let mut content_columns = Vec::new();
    for column in text_columns
        .iter()
        .chain(entity_columns.iter())
        .chain(dimensions.iter())
        .chain(time_dimensions.iter())
        .chain(metrics.iter())
    {
        push_unique_column_if_not(&mut content_columns, column, &id_column);
        if content_columns.len() >= 24 {
            break;
        }
    }
    if content_columns.is_empty() {
        for column in columns {
            push_unique_column_if_not(&mut content_columns, &column.name, &id_column);
            if content_columns.len() >= 12 {
                break;
            }
        }
    }
    if content_columns.is_empty() {
        content_columns.push(id_column.clone());
    }

    let mut metadata_columns = Vec::new();
    for column in dimensions
        .iter()
        .chain(entity_columns.iter())
        .chain(time_dimensions.iter())
        .chain(metrics.iter())
    {
        if column != &id_column {
            push_unique_column(&mut metadata_columns, column);
        }
        if metadata_columns.len() >= 32 {
            break;
        }
    }

    let confidence = mapping_confidence(primary_key_columns, &content_columns, columns);
    DatabaseTableMappingSuggestion {
        table: table.to_string(),
        object_type: DEFAULT_OBJECT_TYPE.to_string(),
        id_column,
        title_column,
        content_columns,
        content_type: DEFAULT_CONTENT_TYPE.to_string(),
        updated_at_column,
        version_column,
        metadata_columns,
        confidence,
        rationale:
            "基于主键、字段名称、数据类型和样本值推断，可作为数据库表同步到数据集文档的默认映射。"
                .to_string(),
    }
}

fn push_unique_column_if_not(columns: &mut Vec<String>, column: &str, excluded: &str) {
    if column != excluded {
        push_unique_column(columns, column);
    }
}

fn mapping_confidence(
    primary_key_columns: &[String],
    content_columns: &[String],
    columns: &[DatabaseColumnSemanticProfile],
) -> u8 {
    let mut confidence = 45u8;
    if !primary_key_columns.is_empty() {
        confidence += 30;
    } else if columns
        .iter()
        .any(|column| looks_like_identifier(&column.name.to_ascii_lowercase()))
    {
        confidence += 15;
    }
    if !content_columns.is_empty() {
        confidence += 20;
    }
    if columns
        .iter()
        .any(|column| column.semantic_role == "metric")
    {
        confidence += 5;
    }
    confidence.min(95)
}

fn table_visualization_suggestions(
    table: &str,
    dimensions: &[String],
    metrics: &[String],
    time_dimensions: &[String],
) -> Vec<DatabaseVisualizationSuggestion> {
    let mut suggestions = Vec::new();
    if let (Some(metric), Some(dimension)) = (metrics.first(), dimensions.first()) {
        suggestions.push(DatabaseVisualizationSuggestion {
            title: format!("按 {dimension} 对 {metric} 排名"),
            chart_type: "bar".to_string(),
            table: table.to_string(),
            dimensions: vec![dimension.clone()],
            metrics: vec![metric.clone()],
            rationale: "适合回答分组汇总、TopN 排名和横向对比问题。".to_string(),
        });
    }
    if let (Some(metric), Some(time_dimension)) = (metrics.first(), time_dimensions.first()) {
        suggestions.push(DatabaseVisualizationSuggestion {
            title: format!("{metric} 随 {time_dimension} 的趋势"),
            chart_type: "line".to_string(),
            table: table.to_string(),
            dimensions: vec![time_dimension.clone()],
            metrics: vec![metric.clone()],
            rationale: "适合观察时间变化、周期波动和增长趋势。".to_string(),
        });
    }
    if let (Some(metric), Some(time_dimension), Some(dimension)) =
        (metrics.first(), time_dimensions.first(), dimensions.first())
    {
        suggestions.push(DatabaseVisualizationSuggestion {
            title: format!("按 {time_dimension} 和 {dimension} 对比 {metric}"),
            chart_type: "heatmap".to_string(),
            table: table.to_string(),
            dimensions: vec![time_dimension.clone(), dimension.clone()],
            metrics: vec![metric.clone()],
            rationale: "适合交叉维度对比，快速定位高低值区域。".to_string(),
        });
    }
    if metrics.is_empty() {
        if let Some(dimension) = dimensions.first() {
            suggestions.push(DatabaseVisualizationSuggestion {
                title: format!("按 {dimension} 统计记录数"),
                chart_type: "bar".to_string(),
                table: table.to_string(),
                dimensions: vec![dimension.clone()],
                metrics: vec!["record_count".to_string()],
                rationale: "没有明显数值指标时，可先做类别分布分析。".to_string(),
            });
        }
    }
    suggestions
}

fn table_suggested_questions(
    table: &str,
    dimensions: &[String],
    metrics: &[String],
    time_dimensions: &[String],
) -> Vec<String> {
    let mut questions = Vec::new();
    if let (Some(metric), Some(dimension)) = (metrics.first(), dimensions.first()) {
        questions.push(format!("按 {dimension} 汇总 {metric} 并给出 Top 10 排名。"));
        questions.push(format!("不同 {dimension} 的 {metric} 差异主要在哪里？"));
    }
    if let (Some(metric), Some(time_dimension)) = (metrics.first(), time_dimensions.first()) {
        questions.push(format!("{metric} 按 {time_dimension} 的趋势如何？"));
    }
    if let Some(metric) = metrics.first() {
        questions.push(format!("找出 {table} 中 {metric} 明显偏高或偏低的记录。"));
    }
    if questions.is_empty() {
        questions.push(format!("概括 {table} 表的主要字段、可用维度和可分析问题。"));
    }
    questions
}

fn table_report_suggestions(table: &DatabaseTableSemanticProfile) -> Vec<DatabaseReportSuggestion> {
    table
        .suggested_visualizations
        .iter()
        .map(|visualization| DatabaseReportSuggestion {
            title: visualization.title.clone(),
            objective: visualization.rationale.clone(),
            table: table.name.clone(),
            visualization: visualization.chart_type.clone(),
            dimensions: visualization.dimensions.clone(),
            metrics: visualization.metrics.clone(),
        })
        .collect()
}

fn is_numeric_type(data_type: &str) -> bool {
    matches!(
        data_type,
        "tinyint"
            | "smallint"
            | "mediumint"
            | "int"
            | "integer"
            | "bigint"
            | "decimal"
            | "numeric"
            | "float"
            | "double"
            | "real"
    )
}

fn is_time_type(data_type: &str) -> bool {
    matches!(
        data_type,
        "date" | "datetime" | "timestamp" | "time" | "year"
    )
}

fn is_text_type(data_type: &str) -> bool {
    matches!(
        data_type,
        "char"
            | "varchar"
            | "tinytext"
            | "text"
            | "mediumtext"
            | "longtext"
            | "enum"
            | "set"
            | "json"
    )
}

fn is_boolean_type(data_type: &str, column_type: &str, name: &str) -> bool {
    data_type == "boolean"
        || data_type == "bool"
        || column_type == "tinyint(1)"
        || name_has_any(
            name,
            &["is_", "has_", "enabled", "disabled", "active", "flag"],
        )
}

fn looks_like_identifier(name: &str) -> bool {
    name == "id"
        || name.ends_with("_id")
        || name.ends_with("_code")
        || name.ends_with("_no")
        || name_has_any(name, &["uuid", "guid", "serial", "number"])
}

fn looks_like_metric(name: &str) -> bool {
    name_has_any(
        name,
        &[
            "count", "num", "amount", "total", "sum", "rate", "ratio", "score", "value", "price",
            "cost", "traffic", "flow", "volume", "qty", "avg", "min", "max", "duration",
            "distance", "area",
        ],
    )
}

fn looks_like_entity(name: &str) -> bool {
    name_has_any(
        name,
        &[
            "user",
            "person",
            "employee",
            "customer",
            "company",
            "org",
            "organization",
            "supplier",
            "vendor",
            "client",
            "project",
            "name",
        ],
    )
}

fn looks_like_dimension(name: &str) -> bool {
    name_has_any(
        name,
        &[
            "type", "status", "category", "class", "level", "gender", "sex", "city", "province",
            "district", "region", "area", "source", "channel", "skill", "tag",
        ],
    )
}

fn name_has_any(name: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| name.contains(needle))
}

fn build_table_preview(
    plan: MySqlTablePreviewQuery,
    rows: Vec<sqlx::mysql::MySqlRow>,
) -> Result<TablePreview, DatabaseSourceError> {
    let mut preview_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let mut object = Map::new();
        for column in &plan.columns {
            let value = row
                .try_get::<Option<String>, _>(column.as_str())
                .map_err(sqlx_error)?;
            object.insert(
                column.clone(),
                value.map(Value::String).unwrap_or(Value::Null),
            );
        }
        preview_rows.push(Value::Object(object));
    }
    Ok(TablePreview {
        table: plan.table,
        row_limit: plan.row_limit,
        columns: plan.columns,
        rows: preview_rows,
    })
}

fn build_aggregate_result(
    plan: MySqlAggregateQuery,
    rows: Vec<sqlx::mysql::MySqlRow>,
) -> Result<DatabaseAggregateResult, DatabaseSourceError> {
    let mut result_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let mut object = Map::new();
        for column in &plan.columns {
            let value = row
                .try_get::<Option<String>, _>(column.as_str())
                .map_err(sqlx_error)?;
            object.insert(
                column.clone(),
                value.map(Value::String).unwrap_or(Value::Null),
            );
        }
        result_rows.push(Value::Object(object));
    }

    Ok(DatabaseAggregateResult {
        table: plan.table,
        dimensions: plan.dimensions,
        metric: plan.metric,
        aggregation: plan.aggregation,
        row_limit: plan.row_limit,
        columns: plan.columns,
        rows: result_rows,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MySqlTablePreviewQuery {
    pub table: String,
    pub row_limit: u32,
    pub columns: Vec<String>,
    pub sql: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MySqlDocumentFetchQuery {
    pub table: String,
    pub row_limit: u32,
    pub columns: Vec<String>,
    pub sql: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MySqlAggregateQuery {
    pub table: String,
    pub dimensions: Vec<String>,
    pub metric: Option<String>,
    pub aggregation: String,
    pub row_limit: u32,
    pub columns: Vec<String>,
    pub sql: String,
}

pub fn build_mysql_table_preview_query(
    config: &MySqlSourceConfig,
    table: &str,
    limit: u32,
) -> Result<MySqlTablePreviewQuery, DatabaseSourceError> {
    config.validate()?;
    validate_identifier("table", table)?;
    let mapping = config
        .tables
        .iter()
        .find(|mapping| mapping.table == table)
        .ok_or_else(|| DatabaseSourceError::InvalidField {
            field: "table",
            reason: "table is not in the configured allowlist".to_string(),
        })?;
    let columns = preview_columns(mapping);
    if columns.is_empty() {
        return Err(DatabaseSourceError::InvalidField {
            field: "columns",
            reason: "at least one preview column is required".to_string(),
        });
    }
    let projections = columns
        .iter()
        .map(|column| {
            let quoted = quote_mysql_identifier(column)?;
            Ok(format!("cast({quoted} as char) as {quoted}"))
        })
        .collect::<Result<Vec<_>, DatabaseSourceError>>()?
        .join(", ");
    let row_limit = limit.clamp(1, config.row_limit.min(MAX_ROW_LIMIT));
    let sql = format!(
        "select {projections} from {} limit {row_limit}",
        quote_mysql_identifier(table)?
    );
    Ok(MySqlTablePreviewQuery {
        table: table.to_string(),
        row_limit,
        columns,
        sql,
    })
}

pub fn build_mysql_aggregate_query(
    config: &MySqlSourceConfig,
    request: &MySqlAggregateRequest,
) -> Result<MySqlAggregateQuery, DatabaseSourceError> {
    config.validate()?;
    validate_identifier("table", &request.table)?;
    let mapping = config
        .tables
        .iter()
        .find(|mapping| mapping.table == request.table)
        .ok_or_else(|| DatabaseSourceError::InvalidField {
            field: "table",
            reason: "table is not in the configured allowlist".to_string(),
        })?;
    let allowed_columns = preview_columns(mapping);
    let dimensions = normalize_identifier_list(request.dimensions.clone());
    if dimensions.len() > 3 {
        return Err(DatabaseSourceError::InvalidField {
            field: "dimensions",
            reason: "at most 3 dimensions are supported".to_string(),
        });
    }
    for dimension in &dimensions {
        validate_allowed_query_column("dimensions", dimension, &allowed_columns)?;
    }

    let aggregation = normalize_aggregate(&request.aggregation)?;
    let metric = request
        .metric
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if aggregation != "count" && metric.is_none() {
        return Err(DatabaseSourceError::InvalidField {
            field: "metric",
            reason: "metric is required for sum, avg, min, and max".to_string(),
        });
    }
    if let Some(metric) = metric.as_deref() {
        validate_allowed_query_column("metric", metric, &allowed_columns)?;
    }

    let mut projections = Vec::new();
    let mut columns = Vec::new();
    for dimension in &dimensions {
        let quoted = quote_mysql_identifier(dimension)?;
        projections.push(format!("cast({quoted} as char) as {quoted}"));
        columns.push(dimension.clone());
    }
    projections.push(format!(
        "{} as `value`",
        aggregate_expression(&aggregation, metric.as_deref())?
    ));
    columns.push("value".to_string());

    let group_by = if dimensions.is_empty() {
        String::new()
    } else {
        format!(
            " group by {}",
            dimensions
                .iter()
                .map(|dimension| quote_mysql_identifier(dimension))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        )
    };
    let order_by = if dimensions.is_empty() {
        String::new()
    } else {
        " order by `value` desc".to_string()
    };
    let row_limit = request
        .limit
        .unwrap_or(50)
        .clamp(1, config.row_limit.min(MAX_ROW_LIMIT));
    let sql = format!(
        "select {} from {}{}{} limit {row_limit}",
        projections.join(", "),
        quote_mysql_identifier(&mapping.table)?,
        group_by,
        order_by
    );

    Ok(MySqlAggregateQuery {
        table: mapping.table.clone(),
        dimensions,
        metric,
        aggregation,
        row_limit,
        columns,
        sql,
    })
}

pub fn build_mysql_document_fetch_query(
    config: &MySqlSourceConfig,
    mapping: &MySqlTableMapping,
) -> Result<MySqlDocumentFetchQuery, DatabaseSourceError> {
    config.validate()?;
    mapping.validate()?;
    let columns = preview_columns(mapping);
    let projections = columns
        .iter()
        .map(|column| {
            let quoted = quote_mysql_identifier(column)?;
            Ok(format!("cast({quoted} as char) as {quoted}"))
        })
        .collect::<Result<Vec<_>, DatabaseSourceError>>()?
        .join(", ");
    let sql = format!(
        "select {projections} from {} limit {}",
        quote_mysql_identifier(&mapping.table)?,
        config.row_limit.min(MAX_ROW_LIMIT)
    );
    Ok(MySqlDocumentFetchQuery {
        table: mapping.table.clone(),
        row_limit: config.row_limit.min(MAX_ROW_LIMIT),
        columns,
        sql,
    })
}

pub fn mysql_schema_tables_query() -> &'static str {
    r#"
select
  cast(table_name as char) as table_name,
  cast(table_type as char) as table_type,
  cast(table_comment as char) as table_comment,
  cast(table_rows as signed) as table_rows,
  date_format(update_time, '%Y-%m-%dT%H:%i:%s') as update_time
from information_schema.tables
where table_schema = ?
order by table_name
"#
}

pub fn mysql_schema_columns_query() -> &'static str {
    r#"
select
  cast(table_name as char) as table_name,
  cast(column_name as char) as column_name,
  ordinal_position as ordinal_position,
  cast(column_default as char) as column_default,
  cast(is_nullable as char) as is_nullable,
  cast(data_type as char) as data_type,
  cast(column_type as char) as column_type,
  cast(column_comment as char) as column_comment
from information_schema.columns
where table_schema = ?
order by table_name, ordinal_position
"#
}

pub fn mysql_schema_statistics_query() -> &'static str {
    r#"
select
  cast(table_name as char) as table_name,
  cast(index_name as char) as index_name,
  cast(column_name as char) as column_name,
  seq_in_index as seq_in_index,
  non_unique as non_unique
from information_schema.statistics
where table_schema = ?
order by table_name, index_name, seq_in_index
"#
}

impl MySqlTableMapping {
    pub fn validate(&self) -> Result<(), DatabaseSourceError> {
        validate_non_empty("table", &self.table)?;
        validate_identifier("table", &self.table)?;
        validate_non_empty("id_column", &self.id_column)?;
        validate_identifier("id_column", &self.id_column)?;

        if let Some(title_column) = self.title_column.as_deref() {
            validate_identifier("title_column", title_column)?;
        }
        if self.content_columns.is_empty() {
            return Err(DatabaseSourceError::InvalidField {
                field: "content_columns",
                reason: "at least one content column is required".to_string(),
            });
        }
        for column in &self.content_columns {
            validate_identifier("content_columns", column)?;
        }
        if let Some(updated_at_column) = self.updated_at_column.as_deref() {
            validate_identifier("updated_at_column", updated_at_column)?;
        }
        if let Some(version_column) = self.version_column.as_deref() {
            validate_identifier("version_column", version_column)?;
        }
        for column in &self.metadata_columns {
            validate_identifier("metadata_columns", column)?;
        }
        validate_non_empty("object_type", &self.object_type)?;
        validate_non_empty("content_type", &self.content_type)?;
        Ok(())
    }

    fn normalize(&mut self) {
        self.table = self.table.trim().to_string();
        self.object_type = self.object_type.trim().to_string();
        self.id_column = self.id_column.trim().to_string();
        self.title_column = normalize_optional_identifier(self.title_column.take());
        self.content_columns = normalize_identifier_list(std::mem::take(&mut self.content_columns));
        self.content_type = self.content_type.trim().to_string();
        self.updated_at_column = normalize_optional_identifier(self.updated_at_column.take());
        self.version_column = normalize_optional_identifier(self.version_column.take());
        self.metadata_columns =
            normalize_identifier_list(std::mem::take(&mut self.metadata_columns));
    }
}

pub fn is_valid_mysql_identifier(identifier: &str) -> bool {
    let value = identifier.trim();
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub fn quote_mysql_identifier(identifier: &str) -> Result<String, DatabaseSourceError> {
    validate_identifier("identifier", identifier)?;
    Ok(format!("`{}`", identifier.trim()))
}

fn unwrap_mysql_source_value(value: &Value) -> &Value {
    value
        .get("mysql_source")
        .or_else(|| value.get("mysqlSource"))
        .or_else(|| value.get("database_source"))
        .or_else(|| value.get("databaseSource"))
        .unwrap_or(value)
}

fn reject_raw_secret_fields(value: &Value) -> Result<(), DatabaseSourceError> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if is_raw_secret_key(key) {
                    return Err(DatabaseSourceError::RawSecretField { field: key.clone() });
                }
                reject_raw_secret_fields(child)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_raw_secret_fields(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_raw_secret_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if normalized.contains("redacted") || normalized == "connectionenv" {
        return false;
    }
    matches!(
        normalized.as_str(),
        "password"
            | "passwd"
            | "pwd"
            | "url"
            | "connectionstring"
            | "connectionurl"
            | "databaseurl"
            | "token"
            | "secret"
            | "apikey"
            | "accesskey"
    ) || normalized.ends_with("token")
        || normalized.ends_with("secret")
        || normalized.ends_with("password")
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), DatabaseSourceError> {
    if value.trim().is_empty() {
        return Err(DatabaseSourceError::MissingRequiredField { field });
    }
    Ok(())
}

fn validate_identifier(field: &'static str, identifier: &str) -> Result<(), DatabaseSourceError> {
    if is_valid_mysql_identifier(identifier) {
        Ok(())
    } else {
        Err(DatabaseSourceError::InvalidIdentifier {
            field,
            identifier: identifier.to_string(),
        })
    }
}

fn is_valid_env_name(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(ch) if ch == '_' || ch.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn normalize_optional_identifier(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_identifier_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn validate_allowed_query_column(
    field: &'static str,
    column: &str,
    allowed_columns: &[String],
) -> Result<(), DatabaseSourceError> {
    validate_identifier(field, column)?;
    if allowed_columns.iter().any(|allowed| allowed == column) {
        Ok(())
    } else {
        Err(DatabaseSourceError::InvalidField {
            field,
            reason: format!("column `{column}` is not in the configured table mapping"),
        })
    }
}

fn normalize_aggregate(value: &str) -> Result<String, DatabaseSourceError> {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(normalized.as_str(), "count" | "sum" | "avg" | "min" | "max") {
        Ok(normalized)
    } else {
        Err(DatabaseSourceError::InvalidField {
            field: "aggregation",
            reason: "must be one of count, sum, avg, min, or max".to_string(),
        })
    }
}

fn aggregate_expression(
    aggregation: &str,
    metric: Option<&str>,
) -> Result<String, DatabaseSourceError> {
    if aggregation == "count" {
        return Ok("cast(count(*) as char)".to_string());
    }
    let metric = metric.ok_or_else(|| DatabaseSourceError::InvalidField {
        field: "metric",
        reason: "metric is required for numeric aggregation".to_string(),
    })?;
    let quoted = quote_mysql_identifier(metric)?;
    let numeric = format!("cast(nullif({quoted}, '') as decimal(30,6))");
    let expression = match aggregation {
        "sum" => format!("cast(coalesce(sum({numeric}), 0) as char)"),
        "avg" => format!("cast(avg({numeric}) as char)"),
        "min" => format!("cast(min({numeric}) as char)"),
        "max" => format!("cast(max({numeric}) as char)"),
        _ => {
            return Err(DatabaseSourceError::InvalidField {
                field: "aggregation",
                reason: "must be one of count, sum, avg, min, or max".to_string(),
            })
        }
    };
    Ok(expression)
}

async fn connect_mysql_pool(config: &MySqlSourceConfig) -> Result<MySqlPool, DatabaseSourceError> {
    let url = std::env::var(&config.connection_env).map_err(|_| {
        DatabaseSourceError::MissingConnectionEnv {
            env: config.connection_env.clone(),
        }
    })?;
    MySqlPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_millis(config.timeout_ms))
        .connect(&url)
        .await
        .map_err(sqlx_error)
}

fn preview_columns(mapping: &MySqlTableMapping) -> Vec<String> {
    let mut columns = Vec::new();
    push_unique_column(&mut columns, &mapping.id_column);
    if let Some(column) = mapping.title_column.as_deref() {
        push_unique_column(&mut columns, column);
    }
    for column in &mapping.content_columns {
        push_unique_column(&mut columns, column);
    }
    if let Some(column) = mapping.updated_at_column.as_deref() {
        push_unique_column(&mut columns, column);
    }
    if let Some(column) = mapping.version_column.as_deref() {
        push_unique_column(&mut columns, column);
    }
    for column in &mapping.metadata_columns {
        push_unique_column(&mut columns, column);
    }
    columns
}

fn mysql_row_to_external_document(
    mapping: &MySqlTableMapping,
    columns: &[String],
    row: sqlx::mysql::MySqlRow,
    include_body: bool,
) -> Result<Value, DatabaseSourceError> {
    let mut values = BTreeMap::new();
    for column in columns {
        let value = row
            .try_get::<Option<String>, _>(column.as_str())
            .map_err(sqlx_error)?
            .unwrap_or_default();
        values.insert(column.clone(), value);
    }

    let primary_key = values
        .get(&mapping.id_column)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| DatabaseSourceError::InvalidField {
            field: "id_column",
            reason: format!(
                "mapped table {} has a row with empty id_column {}",
                mapping.table, mapping.id_column
            ),
        })?
        .to_string();
    let title = mapping
        .title_column
        .as_deref()
        .and_then(|column| values.get(column))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{} #{}", mapping.table, primary_key));
    let body = build_mysql_document_body(mapping, &values, &title, &primary_key);
    let revision_external_id = build_mysql_revision_external_id(mapping, &values, &body);
    let mut metadata = Map::new();
    metadata.insert("source_kind".to_string(), json_string("mysql"));
    metadata.insert("source_table".to_string(), json_string(&mapping.table));
    metadata.insert("source_primary_key".to_string(), json_string(&primary_key));
    metadata.insert("object_type".to_string(), json_string(&mapping.object_type));
    for column in &mapping.metadata_columns {
        if let Some(value) = values.get(column) {
            metadata.insert(column.clone(), json_string(value));
        }
    }

    let mut object = Map::new();
    object.insert(
        "document_external_id".to_string(),
        json_string(&format!("mysql:{}:{}", mapping.table, primary_key)),
    );
    object.insert(
        "revision_external_id".to_string(),
        json_string(&revision_external_id),
    );
    object.insert("title".to_string(), json_string(&title));
    object.insert(
        "content_type".to_string(),
        json_string(&mapping.content_type),
    );
    object.insert("metadata".to_string(), Value::Object(metadata));
    if include_body {
        object.insert("body".to_string(), json_string(&body));
    }
    Ok(Value::Object(object))
}

fn build_mysql_document_body(
    mapping: &MySqlTableMapping,
    values: &BTreeMap<String, String>,
    title: &str,
    primary_key: &str,
) -> String {
    let mut sections = Vec::new();
    for column in &mapping.content_columns {
        if let Some(value) = values
            .get(column)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            sections.push(format!("## {column}\n\n{value}"));
        }
    }
    if sections.is_empty() {
        sections.push(format!(
            "source_table: {}\nsource_primary_key: {}\ntitle: {}",
            mapping.table, primary_key, title
        ));
    }
    sections.join("\n\n")
}

fn build_mysql_revision_external_id(
    mapping: &MySqlTableMapping,
    values: &BTreeMap<String, String>,
    body: &str,
) -> String {
    if let Some(column) = mapping.version_column.as_deref() {
        if let Some(value) = values
            .get(column)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return format!("version:{value}");
        }
    }
    if let Some(column) = mapping.updated_at_column.as_deref() {
        if let Some(value) = values
            .get(column)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return format!("updated_at:{value}");
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(mapping.table.as_bytes());
    hasher.update(b"\0");
    hasher.update(body.as_bytes());
    format!("content_sha256:{:x}", hasher.finalize())
}

fn json_string(value: &str) -> Value {
    Value::String(value.to_string())
}

fn push_unique_column(columns: &mut Vec<String>, column: &str) {
    if !columns.iter().any(|existing| existing == column) {
        columns.push(column.to_string());
    }
}

fn index_views_by_table(
    rows: Vec<sqlx::mysql::MySqlRow>,
) -> Result<BTreeMap<String, Vec<DatabaseIndexView>>, DatabaseSourceError> {
    let mut index_columns: BTreeMap<(String, String), (bool, Vec<(u32, String)>)> = BTreeMap::new();
    for row in rows {
        let table_name: String = row.try_get("table_name").map_err(sqlx_error)?;
        let index_name: String = row.try_get("index_name").map_err(sqlx_error)?;
        let column_name: String = row.try_get("column_name").map_err(sqlx_error)?;
        let seq_in_index: u32 = row.try_get("seq_in_index").map_err(sqlx_error)?;
        let non_unique: u8 = row.try_get("non_unique").map_err(sqlx_error)?;
        let entry = index_columns
            .entry((table_name, index_name))
            .or_insert_with(|| (non_unique == 0, Vec::new()));
        entry.1.push((seq_in_index, column_name));
    }

    let mut by_table: BTreeMap<String, Vec<DatabaseIndexView>> = BTreeMap::new();
    for ((table_name, index_name), (is_unique, mut columns)) in index_columns {
        columns.sort_by_key(|(seq, _)| *seq);
        by_table
            .entry(table_name)
            .or_default()
            .push(DatabaseIndexView {
                name: index_name,
                columns: columns.into_iter().map(|(_, column)| column).collect(),
                is_unique,
            });
    }
    Ok(by_table)
}

fn empty_string_to_none(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn sqlx_error(error: sqlx::Error) -> DatabaseSourceError {
    DatabaseSourceError::Query(error.to_string())
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

fn default_row_limit() -> u32 {
    DEFAULT_ROW_LIMIT
}

fn default_object_type() -> String {
    DEFAULT_OBJECT_TYPE.to_string()
}

fn default_aggregate() -> String {
    "count".to_string()
}

fn default_content_type() -> String {
    DEFAULT_CONTENT_TYPE.to_string()
}

impl fmt::Display for MySqlTableMapping {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_config() -> Value {
        json!({
            "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
            "database": "hy_sql",
            "default_dataset_id": "018f0000-0000-7000-9000-000000000001",
            "tables": [
                {
                    "table": "documents",
                    "id_column": "id",
                    "title_column": "title",
                    "content_columns": [" content ", "summary"],
                    "updated_at_column": "updated_at",
                    "revision_strategy": "updated_at_hash",
                    "metadata_columns": ["category", "owner_id"]
                }
            ]
        })
    }

    #[test]
    fn mysql_source_config_parses_valid_config() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");

        assert_eq!(config.connection_env, "THIRD_PARTY_HY_SQL_DATABASE_URL");
        assert_eq!(config.database, "hy_sql");
        assert_eq!(config.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(config.row_limit, DEFAULT_ROW_LIMIT);
        assert_eq!(
            config.default_dataset_id.as_deref(),
            Some("018f0000-0000-7000-9000-000000000001")
        );
        assert_eq!(config.tables[0].object_type, "document");
        assert_eq!(config.tables[0].content_type, DEFAULT_CONTENT_TYPE);
        assert_eq!(
            config.tables[0].revision_strategy,
            MySqlRevisionStrategy::UpdatedAtHash
        );
        assert_eq!(config.tables[0].content_columns, vec!["content", "summary"]);
    }

    #[test]
    fn mysql_source_config_accepts_wrapped_config() {
        let wrapped = json!({ "mysql_source": valid_config() });

        let config = MySqlSourceConfig::from_value(&wrapped).expect("wrapped config parses");

        assert_eq!(config.database, "hy_sql");
        assert_eq!(config.tables[0].table, "documents");
    }

    #[test]
    fn mysql_source_config_rejects_raw_password() {
        let raw = json!({
            "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
            "password": "secret",
            "database": "hy_sql",
            "tables": []
        });

        let error =
            MySqlSourceConfig::from_value(&raw).expect_err("raw password should not be accepted");

        assert!(error.to_string().contains("raw database secret"));
    }

    #[test]
    fn mysql_source_config_rejects_raw_connection_url() {
        let raw = json!({
            "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
            "database": "hy_sql",
            "connection_string": "mysql://user:password@example/hy_sql",
            "tables": []
        });

        let error = MySqlSourceConfig::from_value(&raw)
            .expect_err("raw connection URL should not be accepted");

        assert!(matches!(error, DatabaseSourceError::RawSecretField { .. }));
    }

    #[test]
    fn mysql_source_config_rejects_token_like_secret_fields() {
        let raw = json!({
            "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
            "database": "hy_sql",
            "access_token": "secret-token",
            "tables": []
        });

        let error =
            MySqlSourceConfig::from_value(&raw).expect_err("raw token should not be accepted");

        assert!(matches!(error, DatabaseSourceError::RawSecretField { .. }));
    }

    #[test]
    fn mysql_source_config_rejects_invalid_identifier() {
        let mut raw = valid_config();
        raw["tables"][0]["table"] = json!("documents;drop");

        let error =
            MySqlSourceConfig::from_value(&raw).expect_err("unsafe table should be rejected");

        assert!(matches!(
            error,
            DatabaseSourceError::InvalidIdentifier { field: "table", .. }
        ));
    }

    #[test]
    fn mysql_source_config_rejects_invalid_column_identifier() {
        let mut raw = valid_config();
        raw["tables"][0]["content_columns"] = json!(["body.text"]);

        let error =
            MySqlSourceConfig::from_value(&raw).expect_err("unsafe column should be rejected");

        assert!(matches!(
            error,
            DatabaseSourceError::InvalidIdentifier {
                field: "content_columns",
                ..
            }
        ));
    }

    #[test]
    fn mysql_source_config_rejects_invalid_default_dataset_id() {
        let mut raw = valid_config();
        raw["default_dataset_id"] = json!("not-a-uuid");

        let error = MySqlSourceConfig::from_value(&raw).expect_err("dataset id must be a uuid");

        assert!(matches!(
            error,
            DatabaseSourceError::InvalidField {
                field: "default_dataset_id",
                ..
            }
        ));
    }

    #[test]
    fn redacted_summary_contains_no_secret_fields() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");

        let summary = config.redacted_summary();
        let encoded = serde_json::to_string(&summary).expect("summary serializes");

        assert_eq!(summary.kind, "mysql");
        assert_eq!(summary.table_count, 1);
        assert!(encoded.contains("THIRD_PARTY_HY_SQL_DATABASE_URL"));
        assert!(!encoded.contains("password"));
        assert!(!encoded.contains("mysql://"));
    }

    #[test]
    fn mysql_identifier_validation_is_strict() {
        assert!(is_valid_mysql_identifier("documents_2026"));
        assert!(is_valid_mysql_identifier("2026_documents"));
        assert!(!is_valid_mysql_identifier(""));
        assert!(!is_valid_mysql_identifier("documents.name"));
        assert!(!is_valid_mysql_identifier("documents-name"));
        assert!(!is_valid_mysql_identifier("documents`name"));
    }

    #[test]
    fn quote_mysql_identifier_requires_safe_identifier() {
        assert_eq!(quote_mysql_identifier("documents").unwrap(), "`documents`");
        assert!(quote_mysql_identifier("documents.name").is_err());
    }

    #[test]
    fn schema_queries_bind_schema_parameter() {
        assert!(mysql_schema_tables_query().contains("where table_schema = ?"));
        assert!(mysql_schema_columns_query().contains("where table_schema = ?"));
        assert!(mysql_schema_statistics_query().contains("where table_schema = ?"));
        assert!(mysql_schema_tables_query().contains("cast(table_name as char) as table_name"));
        assert!(mysql_schema_columns_query().contains("cast(column_name as char) as column_name"));
        assert!(mysql_schema_statistics_query().contains("cast(index_name as char) as index_name"));
    }

    #[test]
    fn table_preview_query_uses_allowlisted_quoted_identifiers() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");

        let plan = build_mysql_table_preview_query(&config, "documents", 5).expect("query builds");

        assert_eq!(plan.table, "documents");
        assert_eq!(plan.row_limit, 5);
        assert_eq!(
            plan.columns,
            vec![
                "id",
                "title",
                "content",
                "summary",
                "updated_at",
                "category",
                "owner_id"
            ]
        );
        assert!(plan.sql.contains("from `documents` limit 5"));
        assert!(plan.sql.contains("cast(`id` as char) as `id`"));
        assert!(!plan.sql.contains(';'));
    }

    #[test]
    fn table_preview_query_rejects_unmapped_table() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");

        let error =
            build_mysql_table_preview_query(&config, "other_documents", 5).expect_err("not mapped");

        assert!(matches!(
            error,
            DatabaseSourceError::InvalidField { field: "table", .. }
        ));
    }

    #[test]
    fn table_preview_query_rejects_unsafe_table() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");

        let error =
            build_mysql_table_preview_query(&config, "documents;drop", 5).expect_err("unsafe");

        assert!(matches!(
            error,
            DatabaseSourceError::InvalidIdentifier { field: "table", .. }
        ));
    }

    #[test]
    fn table_preview_query_clamps_limit_to_configured_row_limit() {
        let mut raw = valid_config();
        raw["row_limit"] = json!(3);
        let config = MySqlSourceConfig::from_value(&raw).expect("config parses");

        let plan =
            build_mysql_table_preview_query(&config, "documents", 100).expect("query builds");

        assert_eq!(plan.row_limit, 3);
        assert!(plan.sql.ends_with("limit 3"));
    }

    #[test]
    fn document_fetch_query_uses_mapping_and_row_limit() {
        let mut raw = valid_config();
        raw["row_limit"] = json!(7);
        let config = MySqlSourceConfig::from_value(&raw).expect("config parses");

        let plan = build_mysql_document_fetch_query(&config, &config.tables[0])
            .expect("document fetch query builds");

        assert_eq!(plan.table, "documents");
        assert_eq!(plan.row_limit, 7);
        assert!(plan.sql.contains("select cast(`id` as char) as `id`"));
        assert!(plan.sql.contains("from `documents` limit 7"));
        assert!(!plan.sql.contains(';'));
    }

    #[test]
    fn aggregate_query_groups_metric_by_dimension() {
        let raw = json!({
            "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
            "database": "hy_sql",
            "row_limit": 100,
            "tables": [{
                "table": "bi_traffic_area",
                "id_column": "id",
                "title_column": "area_name",
                "content_columns": ["area_name", "traffic_count", "stat_date"],
                "metadata_columns": ["traffic_count", "stat_date"]
            }]
        });
        let config = MySqlSourceConfig::from_value(&raw).expect("config parses");
        let request = MySqlAggregateRequest {
            table: "bi_traffic_area".to_string(),
            dimensions: vec!["area_name".to_string()],
            metric: Some("traffic_count".to_string()),
            aggregation: "sum".to_string(),
            limit: Some(10),
        };

        let plan = build_mysql_aggregate_query(&config, &request).expect("aggregate query builds");

        assert_eq!(plan.table, "bi_traffic_area");
        assert_eq!(plan.dimensions, vec!["area_name".to_string()]);
        assert_eq!(plan.metric.as_deref(), Some("traffic_count"));
        assert_eq!(plan.aggregation, "sum");
        assert_eq!(
            plan.columns,
            vec!["area_name".to_string(), "value".to_string()]
        );
        assert!(plan
            .sql
            .contains("select cast(`area_name` as char) as `area_name`"));
        assert!(plan
            .sql
            .contains("sum(cast(nullif(`traffic_count`, '') as decimal(30,6)))"));
        assert!(plan.sql.contains(
            "from `bi_traffic_area` group by `area_name` order by `value` desc limit 10"
        ));
        assert!(!plan.sql.contains(';'));
    }

    #[test]
    fn aggregate_query_supports_count_without_metric() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");
        let request = MySqlAggregateRequest {
            table: "documents".to_string(),
            dimensions: vec!["category".to_string()],
            metric: None,
            aggregation: "count".to_string(),
            limit: Some(5),
        };

        let plan = build_mysql_aggregate_query(&config, &request).expect("count query builds");

        assert_eq!(
            plan.columns,
            vec!["category".to_string(), "value".to_string()]
        );
        assert!(plan.sql.contains("cast(count(*) as char) as `value`"));
        assert!(plan.sql.ends_with("limit 5"));
    }

    #[test]
    fn aggregate_query_rejects_unmapped_columns() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");
        let request = MySqlAggregateRequest {
            table: "documents".to_string(),
            dimensions: vec!["not_mapped".to_string()],
            metric: None,
            aggregation: "count".to_string(),
            limit: Some(5),
        };

        let error = build_mysql_aggregate_query(&config, &request)
            .expect_err("unmapped dimension should be rejected");

        assert!(error
            .to_string()
            .contains("not in the configured table mapping"));
    }

    #[test]
    fn mysql_revision_prefers_updated_at_then_hash() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");
        let mapping = &config.tables[0];
        let mut values = BTreeMap::new();
        values.insert("updated_at".to_string(), "2026-05-21 10:00:00".to_string());

        let revision = build_mysql_revision_external_id(mapping, &values, "body");

        assert_eq!(revision, "updated_at:2026-05-21 10:00:00");

        values.clear();
        let hash_revision = build_mysql_revision_external_id(mapping, &values, "body");
        assert!(hash_revision.starts_with("content_sha256:"));
    }

    #[test]
    fn mysql_document_body_uses_content_sections() {
        let config = MySqlSourceConfig::from_value(&valid_config()).expect("config parses");
        let mapping = &config.tables[0];
        let mut values = BTreeMap::new();
        values.insert("content".to_string(), "第一段".to_string());
        values.insert("summary".to_string(), "摘要".to_string());

        let body = build_mysql_document_body(mapping, &values, "标题", "42");

        assert!(body.contains("## content\n\n第一段"));
        assert!(body.contains("## summary\n\n摘要"));
    }

    #[test]
    fn database_semantic_profile_detects_metrics_dimensions_and_charts() {
        let schema = DatabaseSchemaSnapshot {
            kind: "mysql".to_string(),
            database: "hy_sql".to_string(),
            server_version: "8.0".to_string(),
            tables: vec![DatabaseTableView {
                name: "bi_traffic_area".to_string(),
                table_type: "BASE TABLE".to_string(),
                comment: Some("traffic by area".to_string()),
                approximate_row_count: Some(3),
                update_time: None,
                primary_key_columns: vec!["id".to_string()],
                indexes: Vec::new(),
                columns: vec![
                    DatabaseColumnView {
                        name: "id".to_string(),
                        ordinal_position: 1,
                        data_type: "bigint".to_string(),
                        column_type: "bigint".to_string(),
                        is_nullable: false,
                        default_value: None,
                        comment: None,
                    },
                    DatabaseColumnView {
                        name: "area_name".to_string(),
                        ordinal_position: 2,
                        data_type: "varchar".to_string(),
                        column_type: "varchar(64)".to_string(),
                        is_nullable: false,
                        default_value: None,
                        comment: None,
                    },
                    DatabaseColumnView {
                        name: "traffic_count".to_string(),
                        ordinal_position: 3,
                        data_type: "int".to_string(),
                        column_type: "int".to_string(),
                        is_nullable: true,
                        default_value: None,
                        comment: None,
                    },
                    DatabaseColumnView {
                        name: "stat_date".to_string(),
                        ordinal_position: 4,
                        data_type: "date".to_string(),
                        column_type: "date".to_string(),
                        is_nullable: false,
                        default_value: None,
                        comment: None,
                    },
                ],
            }],
        };
        let preview = TablePreview {
            table: "bi_traffic_area".to_string(),
            row_limit: 3,
            columns: vec![
                "id".to_string(),
                "area_name".to_string(),
                "traffic_count".to_string(),
                "stat_date".to_string(),
            ],
            rows: vec![
                json!({"id": "1", "area_name": "A区", "traffic_count": "10", "stat_date": "2026-05-01"}),
                json!({"id": "2", "area_name": "B区", "traffic_count": "20", "stat_date": "2026-05-01"}),
            ],
        };

        let profile = build_database_semantic_profile(schema, vec![preview]);
        let table = &profile.tables[0];

        assert!(table.dimensions.contains(&"area_name".to_string()));
        assert!(table.metrics.contains(&"traffic_count".to_string()));
        assert!(table.time_dimensions.contains(&"stat_date".to_string()));
        assert_eq!(table.suggested_mapping.table, "bi_traffic_area");
        assert_eq!(table.suggested_mapping.id_column, "id");
        assert_eq!(
            table.suggested_mapping.title_column.as_deref(),
            Some("area_name")
        );
        assert!(table
            .suggested_mapping
            .content_columns
            .contains(&"traffic_count".to_string()));
        assert_eq!(
            table.suggested_mapping.updated_at_column.as_deref(),
            Some("stat_date")
        );
        assert!(table
            .suggested_visualizations
            .iter()
            .any(|item| item.chart_type == "bar"));
        assert!(profile
            .report_suggestions
            .iter()
            .any(|item| item.table == "bi_traffic_area"));
    }

    #[test]
    fn database_semantic_profile_handles_dimension_only_tables() {
        let schema = DatabaseSchemaSnapshot {
            kind: "mysql".to_string(),
            database: "hy_sql".to_string(),
            server_version: "8.0".to_string(),
            tables: vec![DatabaseTableView {
                name: "area_lookup".to_string(),
                table_type: "BASE TABLE".to_string(),
                comment: None,
                approximate_row_count: Some(2),
                update_time: None,
                primary_key_columns: vec!["area_code".to_string()],
                indexes: Vec::new(),
                columns: vec![
                    DatabaseColumnView {
                        name: "area_code".to_string(),
                        ordinal_position: 1,
                        data_type: "varchar".to_string(),
                        column_type: "varchar(32)".to_string(),
                        is_nullable: false,
                        default_value: None,
                        comment: None,
                    },
                    DatabaseColumnView {
                        name: "area_name".to_string(),
                        ordinal_position: 2,
                        data_type: "varchar".to_string(),
                        column_type: "varchar(64)".to_string(),
                        is_nullable: false,
                        default_value: None,
                        comment: None,
                    },
                ],
            }],
        };

        let profile = build_database_semantic_profile(schema, Vec::new());
        let table = &profile.tables[0];

        assert!(table.entity_columns.contains(&"area_code".to_string()));
        assert!(table.dimensions.contains(&"area_name".to_string()));
        assert_eq!(table.suggested_mapping.id_column, "area_code");
        assert!(table
            .suggested_mapping
            .content_columns
            .contains(&"area_name".to_string()));
        assert_eq!(table.suggested_visualizations[0].metrics[0], "record_count");
    }

    #[tokio::test]
    #[ignore]
    async fn live_mysql_schema_inspect() {
        if std::env::var("MYSQL_SOURCE_TEST_ALLOW_LIVE").as_deref() != Ok("true") {
            return;
        }
        let database_name = std::env::var("MYSQL_SOURCE_TEST_DATABASE_NAME")
            .expect("MYSQL_SOURCE_TEST_DATABASE_NAME is required for live smoke");
        let lower_name = database_name.to_ascii_lowercase();
        let allow_non_test =
            std::env::var("MYSQL_SOURCE_TEST_ALLOW_NON_TEST_DATABASE").as_deref() == Ok("true");
        assert!(
            allow_non_test || lower_name.contains("test"),
            "refusing live schema smoke against non-test database without explicit override"
        );
        let raw = json!({
            "connection_env": "MYSQL_SOURCE_TEST_DATABASE_URL",
            "database": database_name,
            "tables": []
        });
        let config = MySqlSourceConfig::from_value(&raw).expect("live config parses");

        let snapshot = inspect_mysql_schema(&config)
            .await
            .expect("live schema inspection succeeds");

        assert_eq!(snapshot.kind, "mysql");
        assert_eq!(snapshot.database, config.database);
    }
}
