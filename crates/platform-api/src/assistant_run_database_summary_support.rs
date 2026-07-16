use crate::{
    assistant_run_database_field_support::{
        assistant_run_database_category_dimensions, assistant_run_database_entity_dimensions,
        assistant_run_database_mapping_field_roles, assistant_run_database_metric_columns,
        assistant_run_database_time_dimensions,
    },
    assistant_run_database_prompt_support::AssistantRunDatabaseAggregatePlan,
    ExternalSourceConnectionSummary,
};
use domain_model::Dataset;
use external_source_connectors::MySqlTableMapping;
use serde_json::{json, Value};

const ASSISTANT_RUN_DATABASE_AGGREGATE_SCAN_LIMIT_DEFAULT: u32 = 5_000;
const ASSISTANT_RUN_DATABASE_AGGREGATE_SCAN_LIMIT_MAX: u32 = 50_000;

pub(crate) fn assistant_run_database_schema_context_item(
    dataset: &Dataset,
    source: &ExternalSourceConnectionSummary,
    mapping: &MySqlTableMapping,
    aggregate_plans: &[AssistantRunDatabaseAggregatePlan],
    metrics: &[String],
    scan_limit: u32,
) -> Value {
    let field_roles = assistant_run_database_mapping_field_roles(mapping);
    let metrics = if metrics.is_empty() {
        assistant_run_database_metric_columns(mapping)
    } else {
        metrics.to_vec()
    };
    let analysis_views = aggregate_plans
        .iter()
        .map(|plan| {
            json!({
                "role": plan.role,
                "intent": plan.intent,
                "dimensions": plan.dimensions,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "type": "database_schema_context",
        "source": "database_source",
        "dataset_id": dataset.id,
        "dataset_key": dataset.key,
        "dataset_title": dataset.title,
        "source_id": source.source_id,
        "connector_kind": source.connector_kind,
        "table": mapping.table,
        "summary": assistant_run_database_schema_summary(mapping, &metrics, aggregate_plans, scan_limit),
        "field_roles": field_roles,
        "entity_dimensions": assistant_run_database_entity_dimensions(mapping),
        "time_dimensions": assistant_run_database_time_dimensions(mapping),
        "category_dimensions": assistant_run_database_category_dimensions(mapping),
        "metrics": metrics,
        "analysis_views": analysis_views,
        "answer_guidance": {
            "scope": "database_source_dataset_mapping",
            "metric_rule": "metrics 仅表示候选数值字段；只有字段语义契约允许的 aggregation 才可执行，回答时说明 aggregation 和 scan_limit；未确认单位、可加性或业务口径时不得自行汇总。",
            "report_rule": "报表优先拆成实体排行、时间趋势、分类对比；不要把不同维度的聚合样本混成同一张图。",
            "scan_limit": scan_limit,
        },
    })
}

pub(crate) fn assistant_run_database_schema_summary(
    mapping: &MySqlTableMapping,
    metrics: &[String],
    aggregate_plans: &[AssistantRunDatabaseAggregatePlan],
    scan_limit: u32,
) -> String {
    let entity_dimensions = assistant_run_database_entity_dimensions(mapping);
    let time_dimensions = assistant_run_database_time_dimensions(mapping);
    let category_dimensions = assistant_run_database_category_dimensions(mapping);
    let view_labels = aggregate_plans
        .iter()
        .map(|plan| match plan.role {
            "ranking" => "实体排行",
            "trend" => "时间趋势",
            "comparison" => "分类对比",
            _ => "聚合分析",
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    format!(
        "数据库表 {}：实体维度 {}；时间维度 {}；分类维度 {}；指标 {}。本轮适合做 {}；聚合扫描上限 {} 行，回答或报表需说明聚合口径和采样范围。",
        mapping.table,
        assistant_run_database_join_or_dash(&entity_dimensions),
        assistant_run_database_join_or_dash(&time_dimensions),
        assistant_run_database_join_or_dash(&category_dimensions),
        assistant_run_database_join_or_dash(metrics),
        assistant_run_database_join_or_dash(&view_labels),
        scan_limit,
    )
}

pub(crate) fn assistant_run_database_aggregate_summary(
    mapping: &MySqlTableMapping,
    role: &str,
    dimensions: &[String],
    metric: Option<&str>,
    aggregation: &str,
    order_direction: &str,
    latest_time_column: Option<&str>,
    row_count: usize,
    scan_limit: Option<u32>,
) -> String {
    let role_label = match role {
        "ranking" => "实体排行",
        "trend" => "时间趋势",
        "comparison" => "分类对比",
        _ => "聚合分析",
    };
    let order_label = if order_direction.eq_ignore_ascii_case("asc") {
        "升序"
    } else {
        "降序"
    };
    let time_filter = latest_time_column
        .map(|column| format!("；时间口径为最新 {column}"))
        .unwrap_or_default();
    let sort_semantics = assistant_run_database_aggregate_sort_semantics(metric, order_direction)
        .map(|note| format!("；{note}"))
        .unwrap_or_default();
    format!(
        "{}：表 {} 按 {} 对 {} 做 {} 聚合并按聚合值{}{}{}，返回 {} 行样本，扫描上限 {}。",
        role_label,
        mapping.table,
        assistant_run_database_join_or_dash(dimensions),
        metric.unwrap_or("record_count"),
        aggregation,
        order_label,
        time_filter,
        sort_semantics,
        row_count,
        scan_limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    )
}

pub(crate) fn assistant_run_database_aggregate_scan_limit() -> u32 {
    std::env::var("ASSISTANT_RUN_DATABASE_AGGREGATE_SCAN_LIMIT")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(ASSISTANT_RUN_DATABASE_AGGREGATE_SCAN_LIMIT_DEFAULT)
        .clamp(1, ASSISTANT_RUN_DATABASE_AGGREGATE_SCAN_LIMIT_MAX)
}

pub(crate) fn assistant_run_database_aggregate_sort_semantics(
    _metric: Option<&str>,
    _order_direction: &str,
) -> Option<&'static str> {
    None
}

pub(crate) fn assistant_run_database_join_or_dash<T: AsRef<str>>(items: &[T]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items
            .iter()
            .map(|item| item.as_ref())
            .collect::<Vec<_>>()
            .join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_run_database_prompt_support::AssistantRunDatabaseAggregatePlan;

    fn traffic_mapping() -> MySqlTableMapping {
        MySqlTableMapping {
            table: "bi_traffic_area".to_string(),
            object_type: "traffic".to_string(),
            id_column: "areacode".to_string(),
            id_columns: vec!["areacode".to_string()],
            title_column: Some("areaname".to_string()),
            content_columns: Vec::new(),
            content_type: "text/markdown".to_string(),
            updated_at_column: Some("txdate".to_string()),
            version_column: None,
            revision_strategy: Default::default(),
            metadata_columns: vec!["areatype".to_string(), "up".to_string(), "down".to_string()],
        }
    }

    #[test]
    fn database_aggregate_sort_semantics_does_not_invent_business_direction() {
        assert_eq!(
            assistant_run_database_aggregate_sort_semantics(Some("quekou"), "asc"),
            None
        );
        assert_eq!(
            assistant_run_database_aggregate_sort_semantics(Some("quekou"), "desc"),
            None
        );
        assert_eq!(
            assistant_run_database_aggregate_sort_semantics(Some("sales"), "asc"),
            None
        );
    }

    #[test]
    fn database_join_or_dash_joins_non_empty_items_and_marks_empty() {
        assert_eq!(
            assistant_run_database_join_or_dash(&["area", "date"]),
            "area/date"
        );
        assert_eq!(assistant_run_database_join_or_dash::<&str>(&[]), "-");
    }

    #[test]
    fn database_schema_summary_mentions_dimensions_metrics_views_and_scan_limit() {
        let mapping = traffic_mapping();
        let plans = vec![
            AssistantRunDatabaseAggregatePlan {
                role: "ranking",
                intent: "entity_topn",
                dimensions: vec!["areaname".to_string()],
            },
            AssistantRunDatabaseAggregatePlan {
                role: "trend",
                intent: "time_series",
                dimensions: vec!["txdate".to_string()],
            },
        ];
        let summary = assistant_run_database_schema_summary(
            &mapping,
            &["up".to_string(), "down".to_string()],
            &plans,
            5000,
        );

        assert!(summary.contains("数据库表 bi_traffic_area"));
        assert!(summary.contains("实体维度 areaname"));
        assert!(summary.contains("时间维度 txdate"));
        assert!(summary.contains("分类维度 areatype"));
        assert!(summary.contains("指标 up/down"));
        assert!(summary.contains("实体排行/时间趋势"));
        assert!(summary.contains("聚合扫描上限 5000 行"));
    }
}
