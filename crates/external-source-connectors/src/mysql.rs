use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
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

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

fn default_row_limit() -> u32 {
    DEFAULT_ROW_LIMIT
}

fn default_object_type() -> String {
    DEFAULT_OBJECT_TYPE.to_string()
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
}
