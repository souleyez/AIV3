use external_source_connectors::MySqlSourceConfig;
use serde_json::{json, Value};

use crate::{
    external_integration_summary::redacted_summary, text_normalization::non_empty_trimmed_string,
    ApiError,
};

pub(crate) fn normalize_external_database_connector_kind(
    kind: &str,
) -> std::result::Result<String, ApiError> {
    let normalized = kind
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' ' | '\t' | '\n' | '\r'))
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
    match normalized.as_str() {
        "" | "mysql" | "mysqlsource" | "databasesource" => Ok("mysql".to_string()),
        "postgresql" | "postgres" | "sqlserver" | "oracle" | "mongodb" | "restapi" => {
            Err(ApiError::bad_request_with_details(
                "unsupported_connector_kind",
                "the first external database-source API version only supports mysql".to_string(),
                json!({
                    "requested_connector_kind": kind,
                    "supported_connector_kinds": ["mysql"],
                }),
            ))
        }
        _ => Err(ApiError::bad_request_with_details(
            "unsupported_connector_kind",
            "connector_kind must be mysql for this endpoint".to_string(),
            json!({
                "requested_connector_kind": kind,
                "supported_connector_kinds": ["mysql"],
            }),
        )),
    }
}

pub(crate) fn normalize_external_database_source_table_names(
    tables: &[String],
) -> std::result::Result<Vec<String>, ApiError> {
    let mut output = Vec::new();
    for table in tables {
        let Some(table) = non_empty_trimmed_string(table) else {
            continue;
        };
        if table.chars().count() > 128 || table.chars().any(char::is_control) {
            return Err(ApiError::bad_request(
                "validation_error",
                "tables must contain printable table names within 128 characters".to_string(),
            ));
        }
        if !output.contains(&table) {
            output.push(table);
        }
    }
    Ok(output)
}

pub(crate) fn infer_database_name_from_connection_url(url: &str) -> Option<String> {
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    let after_scheme = without_query
        .strip_prefix("jdbc:mysql://")
        .or_else(|| without_query.strip_prefix("mysql://"))
        .unwrap_or(without_query);
    after_scheme
        .rsplit('/')
        .next()
        .and_then(non_empty_trimmed_string)
        .and_then(|value| value.split(';').next().and_then(non_empty_trimmed_string))
}

pub(crate) fn redact_database_connection_url(url: &str) -> String {
    let without_query = url.split(['?', '#']).next().unwrap_or(url.trim());
    let Some((scheme, rest)) = without_query.split_once("://") else {
        return "[redacted-database-url]".to_string();
    };
    let rest = rest.rsplit_once('@').map(|(_, host)| host).unwrap_or(rest);
    format!("{scheme}://{rest}")
}

pub(crate) fn external_database_source_config_summary(config: &Value) -> Value {
    let Some(database_source) = source_database_config_fragment(config) else {
        if let Some(pending) = config
            .get("database_source_pending")
            .or_else(|| config.get("databaseSourcePending"))
            .cloned()
        {
            return json!({
                "configured": false,
                "credential_status": "pending_secret_binding",
                "pending": redacted_summary(pending),
            });
        }
        return json!({ "configured": false });
    };
    match MySqlSourceConfig::from_value(&database_source) {
        Ok(config) => serde_json::to_value(config.redacted_summary())
            .unwrap_or_else(|_| json!({ "configured": true, "valid": true })),
        Err(error) => json!({
            "configured": true,
            "valid": false,
            "error": error.to_string(),
        }),
    }
}

pub(crate) fn source_database_config_fragment(source_config_redacted: &Value) -> Option<Value> {
    source_config_redacted
        .get("database_source")
        .or_else(|| source_config_redacted.get("databaseSource"))
        .or_else(|| source_config_redacted.get("mysql_source"))
        .or_else(|| source_config_redacted.get("mysqlSource"))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, response::IntoResponse};

    fn error_status<T>(result: std::result::Result<T, ApiError>) -> Option<StatusCode> {
        Some(result.err()?.into_response().status())
    }

    #[test]
    fn connector_kind_keeps_existing_mysql_aliases_and_rejects_unsupported_kinds() {
        assert_eq!(
            normalize_external_database_connector_kind("").expect("empty should default"),
            "mysql"
        );
        assert_eq!(
            normalize_external_database_connector_kind("My SQL_Source")
                .expect("mysql alias should pass"),
            "mysql"
        );
        assert_eq!(
            normalize_external_database_connector_kind("database-source")
                .expect("database source alias should pass"),
            "mysql"
        );
        assert_eq!(
            error_status(normalize_external_database_connector_kind("postgres")),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            error_status(normalize_external_database_connector_kind("clickhouse")),
            Some(StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn table_names_trim_dedupe_and_reject_control_or_overlong_values() {
        let names = normalize_external_database_source_table_names(&[
            "  bi_contract_warning ".to_string(),
            String::new(),
            "bi_contract_warning".to_string(),
            "sales_daily".to_string(),
        ])
        .expect("valid names should normalize");
        assert_eq!(names, vec!["bi_contract_warning", "sales_daily"]);

        assert_eq!(
            error_status(normalize_external_database_source_table_names(&[
                "bad\nname".to_string()
            ])),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            error_status(normalize_external_database_source_table_names(&[
                "a".repeat(129)
            ])),
            Some(StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn database_name_and_redacted_url_preserve_existing_connection_url_semantics() {
        assert_eq!(
            infer_database_name_from_connection_url(
                "mysql://user:pass@db.example.com:3306/xinbai?ssl=true",
            )
            .as_deref(),
            Some("xinbai")
        );
        assert_eq!(
            infer_database_name_from_connection_url(
                "jdbc:mysql://db.example.com:3306/reporting;useUnicode=true#fragment",
            )
            .as_deref(),
            Some("reporting")
        );
        assert_eq!(
            infer_database_name_from_connection_url("mysql://host/   "),
            None
        );

        assert_eq!(
            redact_database_connection_url("mysql://user:pass@db.example.com:3306/xinbai?token=x"),
            "mysql://db.example.com:3306/xinbai"
        );
        assert_eq!(
            redact_database_connection_url("jdbc:mysql://db.example.com/reporting#secret"),
            "jdbc:mysql://db.example.com/reporting"
        );
        assert_eq!(
            redact_database_connection_url("not-a-url"),
            "[redacted-database-url]"
        );
    }

    #[test]
    fn database_source_config_summary_exposes_mysql_shape_without_secrets() {
        let summary = external_database_source_config_summary(&json!({
            "database_source": {
                "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
                "database": "hy_sql",
                "default_dataset_id": "018f0000-0000-7000-9000-000000000001",
                "tables": [{
                    "table": "bi_traffic_area",
                    "id_column": "id",
                    "content_columns": ["area_name"]
                }]
            }
        }));

        assert_eq!(summary["kind"], json!("mysql"));
        assert_eq!(summary["database"], json!("hy_sql"));
        assert_eq!(
            summary["connection_env"],
            json!("THIRD_PARTY_HY_SQL_DATABASE_URL")
        );
        assert_eq!(summary["table_count"], json!(1));
        assert_eq!(summary["tables"][0], json!("bi_traffic_area"));
        assert!(!summary.to_string().contains("password"));
    }

    #[test]
    fn database_source_config_summary_reports_pending_secret_binding_redacted() {
        let summary = external_database_source_config_summary(&json!({
            "databaseSourcePending": {
                "connection_env": "THIRD_PARTY_PENDING_URL",
                "token": "secret-token",
                "password": "secret-password",
                "tables": ["sales_daily"]
            }
        }));

        assert_eq!(summary["configured"], json!(false));
        assert_eq!(
            summary["credential_status"],
            json!("pending_secret_binding")
        );
        assert_eq!(
            summary["pending"]["connection_env"],
            json!("THIRD_PARTY_PENDING_URL")
        );
        assert!(summary["pending"].get("token").is_none());
        assert!(summary["pending"].get("password").is_none());
        assert!(!summary.to_string().contains("secret-token"));
    }

    #[test]
    fn source_database_config_fragment_accepts_legacy_aliases() {
        for key in [
            "database_source",
            "databaseSource",
            "mysql_source",
            "mysqlSource",
        ] {
            let value = json!({ key: { "database": "hy_sql" } });
            assert_eq!(
                source_database_config_fragment(&value),
                Some(json!({ "database": "hy_sql" }))
            );
        }
        assert_eq!(source_database_config_fragment(&json!({})), None);
    }
}
