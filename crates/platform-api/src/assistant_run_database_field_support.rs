use crate::prompt_match_support::prompt_has_any;
use external_source_connectors::MySqlTableMapping;
use serde_json::{json, Value};

pub(crate) type AssistantRunDatabaseColumnRole = (&'static str, u8);

pub(crate) fn assistant_run_database_mapping_field_roles(
    mapping: &MySqlTableMapping,
) -> Vec<Value> {
    assistant_run_database_mapping_columns(mapping)
        .into_iter()
        .map(|column| assistant_run_database_field_role(mapping, &column))
        .collect()
}

pub(crate) fn assistant_run_database_field_semantics_for_columns(
    mapping: &MySqlTableMapping,
    columns: &[String],
) -> Vec<Value> {
    columns
        .iter()
        .map(|column| {
            if column == "value" {
                json!({
                    "name": column,
                    "role": "aggregate_value",
                    "meaning": "聚合后的指标值",
                    "confidence": 95,
                    "source": "aggregate_result",
                })
            } else {
                assistant_run_database_field_role(mapping, column)
            }
        })
        .collect()
}

fn assistant_run_database_field_role(mapping: &MySqlTableMapping, column: &str) -> Value {
    let (role, confidence) = assistant_run_database_column_role(mapping, column);
    json!({
        "name": column,
        "role": role,
        "meaning": assistant_run_database_field_meaning(column, role),
        "confidence": confidence,
        "source": "mapping_and_name_heuristic",
    })
}

pub(crate) fn assistant_run_database_column_role(
    mapping: &MySqlTableMapping,
    column: &str,
) -> AssistantRunDatabaseColumnRole {
    if mapping.id_column == column {
        return ("primary_key", 96);
    }
    if mapping.title_column.as_deref() == Some(column) {
        return ("entity", 92);
    }
    if assistant_run_database_time_dimensions(mapping)
        .iter()
        .any(|candidate| candidate == column)
    {
        return ("time", 90);
    }
    if mapping
        .id_columns
        .iter()
        .any(|candidate| candidate == column)
    {
        return ("primary_key", 86);
    }
    if assistant_run_database_identifier_column_name(column)
        || assistant_run_database_entity_column_name(column)
    {
        return ("entity", 84);
    }
    if assistant_run_database_metric_column_name(column) {
        return ("metric", 88);
    }
    if assistant_run_database_category_dimensions(mapping)
        .iter()
        .any(|candidate| candidate == column)
    {
        return ("dimension", 84);
    }
    if mapping
        .metadata_columns
        .iter()
        .any(|candidate| candidate == column)
    {
        return ("dimension", 74);
    }
    if mapping
        .content_columns
        .iter()
        .any(|candidate| candidate == column)
    {
        return ("attribute", 64);
    }
    ("unknown", 42)
}

pub(crate) fn assistant_run_database_entity_dimensions(mapping: &MySqlTableMapping) -> Vec<String> {
    let mut dimensions = Vec::new();
    if let Some(title_column) = mapping.title_column.as_deref() {
        push_unique_string(&mut dimensions, title_column);
    }
    for column in assistant_run_database_mapping_columns(mapping) {
        let lower = column.to_ascii_lowercase();
        if lower.contains("name")
            || lower.ends_with("title")
            || assistant_run_database_entity_column_name(&column)
        {
            push_unique_string(&mut dimensions, &column);
        }
        if dimensions.len() >= 4 {
            break;
        }
    }
    if dimensions.is_empty() {
        push_unique_string(&mut dimensions, &mapping.id_column);
    }
    dimensions
}

pub(crate) fn assistant_run_database_time_dimensions(mapping: &MySqlTableMapping) -> Vec<String> {
    let mut dimensions = Vec::new();
    if let Some(time_column) = assistant_run_database_time_column(mapping) {
        push_unique_string(&mut dimensions, &time_column);
    }
    if let Some(updated_at_column) = mapping.updated_at_column.as_deref() {
        push_unique_string(&mut dimensions, updated_at_column);
    }
    dimensions
}

pub(crate) fn assistant_run_database_category_dimensions(
    mapping: &MySqlTableMapping,
) -> Vec<String> {
    let mut dimensions = Vec::new();
    if let Some(category_column) = assistant_run_database_category_dimension(mapping) {
        push_unique_string(&mut dimensions, &category_column);
    }
    let time_dimensions = assistant_run_database_time_dimensions(mapping);
    for column in assistant_run_database_mapping_columns(mapping) {
        if mapping.id_column == column
            || mapping
                .id_columns
                .iter()
                .any(|candidate| candidate == &column)
            || mapping.title_column.as_deref() == Some(column.as_str())
            || time_dimensions.iter().any(|candidate| candidate == &column)
            || assistant_run_database_metric_column_name(&column)
        {
            continue;
        }
        if assistant_run_database_dimension_column_name(&column) {
            push_unique_string(&mut dimensions, &column);
        }
        if dimensions.len() >= 6 {
            break;
        }
    }
    dimensions
}

pub(crate) fn assistant_run_database_metric_columns(mapping: &MySqlTableMapping) -> Vec<String> {
    let mut metrics = Vec::new();
    for column in assistant_run_database_mapping_columns(mapping) {
        if assistant_run_database_column_role(mapping, &column).0 == "metric" {
            push_unique_string(&mut metrics, &column);
        }
        if metrics.len() >= 8 {
            break;
        }
    }
    metrics
}

pub(crate) fn assistant_run_database_category_dimension(
    mapping: &MySqlTableMapping,
) -> Option<String> {
    let columns = assistant_run_database_mapping_columns(mapping);
    columns
        .iter()
        .find(|column| column.eq_ignore_ascii_case("areatype"))
        .cloned()
        .or_else(|| {
            columns
                .iter()
                .find(|column| {
                    let lower = column.to_ascii_lowercase();
                    lower.contains("category")
                        || lower.contains("type")
                        || lower.contains("class")
                        || lower.contains("kind")
                        || lower.contains("status")
                        || lower.contains("level")
                })
                .cloned()
        })
}

pub(crate) fn assistant_run_database_time_column(mapping: &MySqlTableMapping) -> Option<String> {
    let columns = assistant_run_database_mapping_columns(mapping);
    columns
        .iter()
        .find(|column| column.eq_ignore_ascii_case("txdate"))
        .cloned()
        .or_else(|| {
            columns
                .iter()
                .find(|column| {
                    let lower = column.to_ascii_lowercase();
                    lower.contains("date") || lower.contains("time")
                })
                .cloned()
        })
}

pub(crate) fn assistant_run_database_mapping_columns(mapping: &MySqlTableMapping) -> Vec<String> {
    let mut columns = Vec::new();
    push_unique_string(&mut columns, &mapping.id_column);
    for column in &mapping.id_columns {
        push_unique_string(&mut columns, column);
    }
    if let Some(column) = mapping.title_column.as_deref() {
        push_unique_string(&mut columns, column);
    }
    for column in &mapping.content_columns {
        push_unique_string(&mut columns, column);
    }
    if let Some(column) = mapping.updated_at_column.as_deref() {
        push_unique_string(&mut columns, column);
    }
    if let Some(column) = mapping.version_column.as_deref() {
        push_unique_string(&mut columns, column);
    }
    for column in &mapping.metadata_columns {
        push_unique_string(&mut columns, column);
    }
    columns
}

fn push_unique_string(items: &mut Vec<String>, item: &str) {
    let item = item.trim();
    if item.is_empty() {
        return;
    }
    if !items.iter().any(|existing| existing == item) {
        items.push(item.to_string());
    }
}

pub(crate) fn assistant_run_database_field_meaning(column: &str, role: &str) -> String {
    let lower = column.to_ascii_lowercase();
    if lower == "up" {
        return "上行/进入/进场方向的流量指标，具体业务口径需以客户定义为准。".to_string();
    }
    if lower == "down" {
        return "下行/离开/出场方向的流量指标，具体业务口径需以客户定义为准。".to_string();
    }
    if lower == "areaname" || lower.ends_with("area_name") || lower.contains("area_name") {
        return "区域或位置名称，适合作为排行、对比和筛选维度。".to_string();
    }
    if lower == "areatype" || lower.contains("area_type") {
        return "区域类型或分类，适合作为分类对比维度。".to_string();
    }
    if lower == "txdate" || lower.contains("date") || lower.contains("time") {
        return "业务发生时间或统计时间，适合作为趋势维度。".to_string();
    }
    if lower.ends_with("code") || lower.ends_with("_code") {
        return "业务编码字段，通常用于唯一识别或关联，不宜直接作为展示名称。".to_string();
    }
    if lower.ends_with("id") || lower.ends_with("_id") {
        return "业务 ID 字段，通常用于唯一识别或关联。".to_string();
    }
    match role {
        "metric" => "候选数值字段；可用聚合方式、单位与业务口径需经字段语义契约确认。".to_string(),
        "time" => "时间维度，可用于趋势、周期和时间范围分析。".to_string(),
        "entity" => "实体名称或对象名称，适合展示、排行和分组。".to_string(),
        "dimension" => "分类或属性维度，适合分组、筛选和对比。".to_string(),
        "primary_key" => "主键或近似主键，主要用于定位记录。".to_string(),
        "attribute" => "同步到数据集文档的属性字段，可用于补充上下文。".to_string(),
        _ => "字段语义暂不明确，使用时应结合样本值或业务说明确认。".to_string(),
    }
}

pub(crate) fn assistant_run_database_metric_column_name(column: &str) -> bool {
    let lower = column.to_ascii_lowercase();
    if assistant_run_database_identifier_column_name(&lower) {
        return false;
    }
    matches!(
        lower.as_str(),
        "up" | "down"
            | "amttotal"
            | "amtsold"
            | "salenum"
            | "xuzengxiaoshou"
            | "quekou"
            | "tichengzujin"
            | "yuezujin"
            | "htzj"
            | "fdxshje"
            | "yze"
            | "rdj"
    ) || assistant_run_database_field_name_has_any_segment(
        &lower,
        &[
            "count", "num", "amount", "total", "sum", "rate", "ratio", "score", "price", "cost",
            "traffic", "flow", "volume", "qty", "avg", "duration", "amt", "sale", "sales", "sold",
            "rent", "fee", "zujin", "xiaoshou", "quekou", "jine", "ticheng", "dayamt", "yuezu",
            "fdxshje", "yze", "rdj",
        ],
    )
}

fn assistant_run_database_field_name_has_any_segment(name: &str, segments: &[&str]) -> bool {
    name.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .any(|segment| segments.contains(&segment))
}

pub(crate) fn assistant_run_database_identifier_column_name(column: &str) -> bool {
    let lower = column.trim().to_ascii_lowercase();
    lower == "id"
        || lower == "key"
        || lower == "uuid"
        || lower == "guid"
        || lower == "serial"
        || lower == "number"
        || (lower.len() > 2 && lower.ends_with("id"))
        || (lower.len() > 4 && lower.ends_with("code"))
        || (lower.len() > 3 && lower.ends_with("key"))
        || lower.ends_with("_id")
        || lower.ends_with("_code")
        || lower.ends_with("_no")
        || lower.ends_with("number")
        || lower.ends_with("serial")
}

pub(crate) fn assistant_run_database_entity_column_name(column: &str) -> bool {
    let lower = column.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "shopdesc"
            | "shopname"
            | "shop_name"
            | "storedesc"
            | "store_desc"
            | "storename"
            | "store_name"
            | "branddesc"
            | "brand_name"
            | "brandname"
            | "leasename"
            | "lease_name"
    )
}

pub(crate) fn assistant_run_database_dimension_column_name(column: &str) -> bool {
    let lower = column.to_ascii_lowercase();
    prompt_has_any(
        &lower,
        &[
            "type", "status", "category", "class", "level", "gender", "city", "province",
            "district", "region", "area", "source", "channel", "tag",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_field_support_keeps_metric_entity_dimension_and_meaning_rules() {
        assert!(assistant_run_database_metric_column_name("quekou"));
        assert!(assistant_run_database_metric_column_name("up"));
        assert!(!assistant_run_database_metric_column_name("parentcode"));
        assert!(!assistant_run_database_metric_column_name("storecode"));
        assert!(!assistant_run_database_metric_column_name("contract_no"));
        assert!(!assistant_run_database_metric_column_name(
            "customer_number"
        ));
        assert!(!assistant_run_database_metric_column_name("currentstatus"));
        assert!(!assistant_run_database_metric_column_name("status_value"));
        assert!(!assistant_run_database_metric_column_name("price_key"));
        assert!(!assistant_run_database_metric_column_name("count_key"));
        assert!(assistant_run_database_entity_column_name("shopdesc"));
        assert!(assistant_run_database_dimension_column_name("areatype"));
        assert!(assistant_run_database_field_meaning("txdate", "time").contains("趋势维度"));
        let metric_meaning = assistant_run_database_field_meaning("unknown", "metric");
        assert!(metric_meaning.contains("候选数值字段"));
        assert!(metric_meaning.contains("字段语义契约"));
    }

    #[test]
    fn database_metric_candidates_exclude_mapping_identity_columns_before_name_heuristics() {
        let mapping = MySqlTableMapping {
            table: "bi_contract_warning".to_string(),
            object_type: "document".to_string(),
            id_column: "parentcode".to_string(),
            id_columns: vec![
                "parentcode".to_string(),
                "storecode".to_string(),
                "txdate".to_string(),
            ],
            title_column: Some("shopdesc".to_string()),
            content_columns: vec![
                "parentcode".to_string(),
                "shopdesc".to_string(),
                "yuezujin".to_string(),
            ],
            content_type: "text/markdown".to_string(),
            updated_at_column: Some("txdate".to_string()),
            version_column: None,
            revision_strategy: Default::default(),
            metadata_columns: vec![
                "parentcode".to_string(),
                "storecode".to_string(),
                "yuezujin".to_string(),
            ],
        };

        assert_eq!(
            assistant_run_database_metric_columns(&mapping),
            vec!["yuezujin".to_string()]
        );
        assert_eq!(
            assistant_run_database_column_role(&mapping, "txdate").0,
            "time"
        );
    }
}
