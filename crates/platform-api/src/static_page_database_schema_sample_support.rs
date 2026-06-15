use crate::assistant_run_text_support::{collect_string_list, push_string_hint};
use crate::static_page_artifact_summary_support::static_page_artifact_string;
use crate::static_page_database_aggregate_sample_support::static_page_database_aggregate_module_text;
use crate::static_page_evidence_signal_support::static_page_text_contains_any;
use serde_json::{json, Value};

pub(crate) fn build_static_page_database_schema_sample_points(
    evidence_items: &[Value],
    module: &Value,
    field_path: Option<&str>,
) -> Vec<Value> {
    let requested_schema = field_path.and_then(static_page_database_schema_field_path_parts);
    if requested_schema.is_none()
        && !static_page_module_requests_database_schema_overview(module, field_path)
    {
        return Vec::new();
    }

    static_page_database_schema_sample_points(evidence_items, requested_schema.as_ref(), field_path)
}

pub(crate) fn static_page_database_schema_sample_points(
    evidence_items: &[Value],
    requested_schema: Option<&(String, String)>,
    field_path: Option<&str>,
) -> Vec<Value> {
    let mut points = Vec::new();
    for item in evidence_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("database_schema_context"))
    {
        let table =
            static_page_artifact_string(item, &["table"]).unwrap_or_else(|| "database".to_string());
        if let Some((requested_table, requested_group)) = requested_schema {
            if requested_table != &table {
                continue;
            }
            let fields = static_page_database_schema_group_fields(item, requested_group);
            if fields.is_empty() {
                continue;
            }
            points.push(static_page_database_schema_group_sample_point(
                item,
                &table,
                requested_group,
                fields,
                field_path,
            ));
        } else {
            points.push(static_page_database_schema_overview_sample_point(
                item, &table,
            ));
        }
        if points.len() >= 8 {
            break;
        }
    }
    points
}

fn static_page_module_requests_database_schema_overview(
    module: &Value,
    field_path: Option<&str>,
) -> bool {
    if field_path.is_some_and(|path| path.starts_with("database.schema")) {
        return true;
    }
    let module_text =
        static_page_database_aggregate_module_text(module, field_path).to_ascii_lowercase();
    static_page_text_contains_any(
        &module_text,
        &[
            "数据库",
            "数据表",
            "结构",
            "字段",
            "指标",
            "维度",
            "口径",
            "schema",
            "table",
            "field",
            "metric",
            "dimension",
        ],
    )
}

pub(crate) fn static_page_database_schema_field_path_parts(
    field_path: &str,
) -> Option<(String, String)> {
    let mut parts = field_path.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("database"), Some("schema"), Some(table), Some(group))
            if !table.trim().is_empty() && !group.trim().is_empty() =>
        {
            Some((table.trim().to_string(), group.trim().to_string()))
        }
        _ => None,
    }
}

pub(crate) fn static_page_database_schema_group_fields(item: &Value, group: &str) -> Vec<String> {
    let mut fields = Vec::new();
    if group == "field_roles" {
        if let Some(roles) = item.get("field_roles").and_then(Value::as_array) {
            for role in roles {
                if let Some(name) = role.get("name").and_then(Value::as_str) {
                    push_string_hint(&mut fields, name);
                }
            }
        }
        return fields;
    }
    if let Some(value) = item.get(group) {
        collect_string_list(value, &mut fields);
    }
    fields
}

fn static_page_database_schema_group_sample_point(
    item: &Value,
    table: &str,
    group: &str,
    fields: Vec<String>,
    field_path: Option<&str>,
) -> Value {
    json!({
        "label": format!("{table} {}", static_page_database_schema_group_label(group)),
        "value": fields.len() as f64,
        "kind": "database_schema",
        "source": "database_schema",
        "fieldPath": field_path.map(ToString::to_string).unwrap_or_else(|| format!("database.schema.{table}.{group}")),
        "datasetId": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "sourceId": item.get("source_id").cloned().unwrap_or(Value::Null),
        "table": table,
        "fieldGroup": group,
        "fields": fields,
        "summary": item.get("summary").cloned().unwrap_or(Value::Null),
        "text": static_page_database_schema_group_text(item, table, group),
    })
}

fn static_page_database_schema_overview_sample_point(item: &Value, table: &str) -> Value {
    let metrics = static_page_database_schema_group_fields(item, "metrics");
    let entity_dimensions = static_page_database_schema_group_fields(item, "entity_dimensions");
    let time_dimensions = static_page_database_schema_group_fields(item, "time_dimensions");
    let category_dimensions = static_page_database_schema_group_fields(item, "category_dimensions");
    let field_count =
        metrics.len() + entity_dimensions.len() + time_dimensions.len() + category_dimensions.len();
    json!({
        "label": table,
        "value": field_count as f64,
        "kind": "database_schema",
        "source": "database_schema",
        "fieldPath": format!("database.schema.{table}.overview"),
        "datasetId": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "sourceId": item.get("source_id").cloned().unwrap_or(Value::Null),
        "table": table,
        "summary": item.get("summary").cloned().unwrap_or(Value::Null),
        "fieldGroups": {
            "metrics": metrics,
            "entityDimensions": entity_dimensions,
            "timeDimensions": time_dimensions,
            "categoryDimensions": category_dimensions,
        },
        "text": item
            .get("summary")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("数据库表 {table} 的结构语义。")),
    })
}

fn static_page_database_schema_group_label(group: &str) -> &'static str {
    match group {
        "metrics" => "指标字段",
        "entity_dimensions" => "实体维度",
        "time_dimensions" => "时间维度",
        "category_dimensions" => "分类维度",
        "field_roles" => "字段角色",
        _ => "字段",
    }
}

fn static_page_database_schema_group_text(item: &Value, table: &str, group: &str) -> String {
    let fields = static_page_database_schema_group_fields(item, group);
    let fields = if fields.is_empty() {
        "-".to_string()
    } else {
        fields.join(" / ")
    };
    format!(
        "数据库表 {table} 的{}：{fields}。",
        static_page_database_schema_group_label(group)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn database_schema_field_path_parts_parses_valid_paths() {
        assert_eq!(
            static_page_database_schema_field_path_parts("database.schema.bi_sales.metrics"),
            Some(("bi_sales".to_string(), "metrics".to_string()))
        );
        assert_eq!(
            static_page_database_schema_field_path_parts("database.schema..metrics"),
            None
        );
        assert_eq!(
            static_page_database_schema_field_path_parts("database.aggregate.bi_sales.rows"),
            None
        );
    }

    #[test]
    fn database_schema_sample_points_build_overview_points_and_cap_at_eight() {
        let evidence_items = (0..10)
            .map(|index| {
                json!({
                    "type": "database_schema_context",
                    "dataset_id": "dataset-1",
                    "source_id": "hy-sql",
                    "table": format!("table_{index}"),
                    "summary": format!("表 {index}"),
                    "metrics": ["sales", "rent"],
                    "entity_dimensions": ["store"],
                    "time_dimensions": ["txdate"],
                    "category_dimensions": ["category"]
                })
            })
            .collect::<Vec<_>>();

        let points = static_page_database_schema_sample_points(&evidence_items, None, None);

        assert_eq!(points.len(), 8);
        assert_eq!(points[0].get("label"), Some(&json!("table_0")));
        assert_eq!(points[0].get("value"), Some(&json!(5.0)));
        assert_eq!(points[0].get("kind"), Some(&json!("database_schema")));
        assert_eq!(
            points[0].get("fieldPath"),
            Some(&json!("database.schema.table_0.overview"))
        );
        assert_eq!(
            points[0].pointer("/fieldGroups/entityDimensions/0"),
            Some(&json!("store"))
        );
    }

    #[test]
    fn database_schema_sample_points_filter_to_requested_group() {
        let requested = ("bi_sales".to_string(), "metrics".to_string());

        let points = static_page_database_schema_sample_points(
            &[schema_item("bi_sales"), schema_item("bi_store")],
            Some(&requested),
            Some("database.schema.bi_sales.metrics"),
        );

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("bi_sales 指标字段")));
        assert_eq!(
            points[0].get("fieldPath"),
            Some(&json!("database.schema.bi_sales.metrics"))
        );
        assert_eq!(points[0].get("fieldGroup"), Some(&json!("metrics")));
        assert_eq!(points[0].pointer("/fields/0"), Some(&json!("sales")));
        assert_eq!(
            points[0].get("text"),
            Some(&json!("数据库表 bi_sales 的指标字段：sales / rent。"))
        );
    }

    #[test]
    fn database_schema_group_fields_support_field_roles() {
        let item = json!({
            "field_roles": [
                {"name": "门店"},
                {"name": "门店"},
                {"name": "销售额"}
            ]
        });

        assert_eq!(
            static_page_database_schema_group_fields(&item, "field_roles"),
            vec!["门店".to_string(), "销售额".to_string()]
        );
    }

    #[test]
    fn database_schema_sample_points_skip_empty_requested_group() {
        let requested = ("bi_sales".to_string(), "time_dimensions".to_string());
        let item = json!({
            "type": "database_schema_context",
            "table": "bi_sales",
            "time_dimensions": []
        });

        let points = static_page_database_schema_sample_points(&[item], Some(&requested), None);

        assert!(points.is_empty());
    }

    #[test]
    fn database_schema_build_wrapper_skips_plain_modules_without_schema_intent() {
        let module = json!({
            "title": "普通经营说明",
            "visualization": {"type": "text-insight"}
        });

        let points = build_static_page_database_schema_sample_points(
            &[schema_item("bi_sales")],
            &module,
            None,
        );

        assert!(points.is_empty());
    }

    #[test]
    fn database_schema_build_wrapper_supports_schema_overview_intent() {
        let module = json!({
            "title": "数据库字段口径",
            "visualization": {"type": "text-insight"}
        });

        let points = build_static_page_database_schema_sample_points(
            &[schema_item("bi_sales")],
            &module,
            None,
        );

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("bi_sales")));
        assert_eq!(
            points[0].get("fieldPath"),
            Some(&json!("database.schema.bi_sales.overview"))
        );
    }

    #[test]
    fn database_schema_build_wrapper_filters_explicit_field_path() {
        let module = json!({
            "title": "指标字段",
            "visualization": {"type": "bar-chart"},
            "dataBinding": {"fieldPath": "database.schema.bi_sales.metrics"}
        });

        let points = build_static_page_database_schema_sample_points(
            &[schema_item("bi_sales"), schema_item("bi_store")],
            &module,
            Some("database.schema.bi_sales.metrics"),
        );

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("bi_sales 指标字段")));
        assert_eq!(points[0].get("fieldGroup"), Some(&json!("metrics")));
        assert_eq!(
            points[0].get("fieldPath"),
            Some(&json!("database.schema.bi_sales.metrics"))
        );
    }

    fn schema_item(table: &str) -> Value {
        json!({
            "type": "database_schema_context",
            "dataset_id": "dataset-1",
            "source_id": "hy-sql",
            "table": table,
            "summary": format!("{table} 表"),
            "metrics": ["sales", "rent"],
            "entity_dimensions": ["store"],
            "time_dimensions": ["txdate"],
            "category_dimensions": ["category"]
        })
    }
}
