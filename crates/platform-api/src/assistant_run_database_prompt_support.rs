use crate::{
    assistant_run_database_field_support::{
        assistant_run_database_category_dimension, assistant_run_database_column_role,
        assistant_run_database_identifier_column_name, assistant_run_database_mapping_columns,
        assistant_run_database_time_column,
    },
    prompt_match_support::{contains_ascii_token, prompt_has_any, prompt_has_metric_terms},
};
use external_source_connectors::{MySqlSourceConfig, MySqlTableMapping};

const ASSISTANT_RUN_DATABASE_SCHEMA_TABLE_LIMIT: usize = 4;
pub(crate) const ASSISTANT_RUN_DATABASE_AGGREGATE_METRIC_LIMIT: usize = 2;
pub(crate) const ASSISTANT_RUN_DATABASE_AGGREGATE_REQUEST_LIMIT: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssistantRunDatabaseAggregatePlan {
    pub(crate) role: &'static str,
    pub(crate) intent: &'static str,
    pub(crate) dimensions: Vec<String>,
}

pub(crate) fn assistant_run_database_mapping_for_prompt<'a>(
    config: &'a MySqlSourceConfig,
    prompt: &str,
) -> Option<&'a MySqlTableMapping> {
    let mut best_match = None;
    let mut best_score = 0usize;
    let mut best_score_tied = false;
    for mapping in &config.tables {
        let score = assistant_run_database_mapping_prompt_score(mapping, prompt);
        if score > best_score {
            best_score = score;
            best_match = Some(mapping);
            best_score_tied = false;
        } else if score > 0 && score == best_score {
            best_score_tied = true;
        }
    }
    if best_score_tied {
        None
    } else {
        best_match
    }
}

pub(crate) fn assistant_run_database_schema_mappings_for_prompt<'a>(
    config: &'a MySqlSourceConfig,
    prompt: &str,
) -> Vec<&'a MySqlTableMapping> {
    let mut mappings = config
        .tables
        .iter()
        .filter(|mapping| assistant_run_database_mapping_prompt_score(mapping, prompt) > 0)
        .take(ASSISTANT_RUN_DATABASE_SCHEMA_TABLE_LIMIT)
        .collect::<Vec<_>>();
    if mappings.is_empty() && assistant_run_database_schema_overview_requested(prompt) {
        // This fallback is schema-only: it lets an explicit schema overview list a bounded
        // set of configured tables. Aggregate planning uses mapping_for_prompt and has no
        // first-table fallback.
        mappings = config
            .tables
            .iter()
            .take(ASSISTANT_RUN_DATABASE_SCHEMA_TABLE_LIMIT)
            .collect();
    }
    mappings
}

pub(crate) fn assistant_run_database_schema_overview_requested(prompt: &str) -> bool {
    let concept_or_tutorial = prompt_has_any(
        prompt,
        &[
            "什么是",
            "是什么意思",
            "怎么做",
            "如何做",
            "怎么查看",
            "如何查看",
            "设计原则",
            "最佳实践",
            "how to",
            "what is",
            "what does",
            "tutorial",
            "best practice",
            "design principle",
        ],
    );
    if concept_or_tutorial {
        return false;
    }
    let explicit_listing = prompt_has_any(
        prompt,
        &[
            "有哪些表",
            "有什么表",
            "哪些表",
            "列出表",
            "列一下表",
            "展示表",
            "查看表结构",
            "看看表结构",
            "表结构概览",
            "数据库结构概览",
            "数据源结构概览",
            "有哪些字段",
            "有什么字段",
            "哪些字段",
            "列出字段",
            "列一下字段",
            "展示字段",
            "字段分别代表",
            "字段含义",
            "字段说明",
            "有哪些数据可以用",
            "有什么数据可以用",
            "哪些数据可以用",
            "可用的数据",
            "可用数据",
            "schema overview",
            "show database schema",
            "list database schema",
            "show source schema",
            "list tables",
            "show tables",
            "list fields",
            "show fields",
            "available fields",
            "available tables",
            "what tables",
            "which tables",
            "what fields",
            "which fields",
        ],
    );
    if explicit_listing {
        return true;
    }
    prompt_has_any(
        prompt,
        &["查看数据库结构", "看看数据库结构", "展示数据库结构"],
    )
}

fn assistant_run_database_mapping_prompt_score(mapping: &MySqlTableMapping, prompt: &str) -> usize {
    let normalized = prompt.to_ascii_lowercase();
    let mut score = 0usize;
    if mapping.table.trim().chars().count() > 1
        && assistant_run_database_prompt_mentions_identifier(&normalized, &mapping.table)
    {
        score += 1_000;
    }
    for column in assistant_run_database_mapping_columns(mapping) {
        if assistant_run_database_mapping_selection_column_is_meaningful(mapping, &column, prompt)
            && assistant_run_database_prompt_mentions_identifier(&normalized, &column)
        {
            score += 10;
        }
    }
    score += assistant_run_database_explicit_metric_columns(mapping, prompt).len() * 100;
    score
}

fn assistant_run_database_mapping_selection_column_is_meaningful(
    mapping: &MySqlTableMapping,
    column: &str,
    prompt: &str,
) -> bool {
    let column = column.trim();
    if column.chars().count() <= 1
        || mapping.id_column.eq_ignore_ascii_case(column)
        || mapping
            .id_columns
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(column))
        || mapping
            .title_column
            .as_deref()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(column))
        || mapping
            .updated_at_column
            .as_deref()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(column))
        || mapping
            .version_column
            .as_deref()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(column))
        || assistant_run_database_identifier_column_name(column)
    {
        return false;
    }
    if matches!(column.to_ascii_lowercase().as_str(), "up" | "down")
        && !assistant_run_database_ascii_direction_metric_context(mapping, prompt)
    {
        return false;
    }
    matches!(
        assistant_run_database_column_role(mapping, column).0,
        "metric" | "dimension" | "attribute" | "entity"
    )
}

fn assistant_run_database_prompt_mentions_identifier(
    normalized_prompt: &str,
    identifier: &str,
) -> bool {
    let identifier = identifier.trim().to_ascii_lowercase();
    !identifier.is_empty() && contains_ascii_token(normalized_prompt, &identifier)
}

pub(crate) fn assistant_run_database_aggregate_dimension_plans(
    mapping: &MySqlTableMapping,
    prompt: &str,
) -> Vec<AssistantRunDatabaseAggregatePlan> {
    let wants_report = assistant_run_database_prompt_wants_report(prompt);
    let wants_rank = wants_report
        || prompt_has_any(
            prompt,
            &[
                "区域", "位置", "楼层", "门", "梯", "点位", "areaname", "area", "top", "排名",
                "排行", "排序", "前", "最高", "最大", "哪些", "哪个", "店铺", "门店", "分店",
                "品牌",
            ],
        );
    let wants_trend = wants_report
        || prompt_has_any(
            prompt,
            &[
                "时间", "日期", "小时", "日", "趋势", "变化", "txdate", "date", "time", "by time",
            ],
        );
    let wants_comparison = wants_report
        || prompt_has_any(
            prompt,
            &[
                "对比", "分类", "类型", "类别", "维度", "areatype", "category", "type",
            ],
        );

    let mut plans = Vec::new();
    if wants_rank {
        if let Some(dimension) = assistant_run_database_prompt_entity_dimension(mapping, prompt)
            .or_else(|| assistant_run_database_entity_dimension(mapping))
        {
            push_assistant_run_database_aggregate_plan(
                &mut plans,
                "ranking",
                "entity_topn",
                vec![dimension],
            );
        }
    }
    if wants_trend {
        if let Some(dimension) = assistant_run_database_time_column(mapping) {
            push_assistant_run_database_aggregate_plan(
                &mut plans,
                "trend",
                "time_series",
                vec![dimension],
            );
        }
    }
    if wants_comparison {
        if let Some(dimension) = assistant_run_database_category_dimension(mapping) {
            push_assistant_run_database_aggregate_plan(
                &mut plans,
                "comparison",
                "category_comparison",
                vec![dimension],
            );
        }
    }
    if plans.is_empty() {
        push_assistant_run_database_aggregate_plan(
            &mut plans,
            "primary",
            "prompt_requested",
            assistant_run_database_aggregate_dimensions(mapping, prompt),
        );
    }
    plans.truncate(ASSISTANT_RUN_DATABASE_AGGREGATE_REQUEST_LIMIT);
    plans
}

fn push_assistant_run_database_aggregate_plan(
    plans: &mut Vec<AssistantRunDatabaseAggregatePlan>,
    role: &'static str,
    intent: &'static str,
    dimensions: Vec<String>,
) {
    if dimensions.is_empty()
        || plans
            .iter()
            .any(|existing| existing.dimensions == dimensions)
    {
        return;
    }
    plans.push(AssistantRunDatabaseAggregatePlan {
        role,
        intent,
        dimensions,
    });
}

fn assistant_run_database_entity_dimension(mapping: &MySqlTableMapping) -> Option<String> {
    mapping
        .title_column
        .as_deref()
        .map(ToOwned::to_owned)
        .or_else(|| Some(mapping.id_column.clone()))
}

fn assistant_run_database_prompt_entity_dimension(
    mapping: &MySqlTableMapping,
    prompt: &str,
) -> Option<String> {
    let columns = assistant_run_database_mapping_columns(mapping);
    let find_column = |patterns: &[&str]| {
        patterns.iter().find_map(|pattern| {
            columns
                .iter()
                .find(|column| column.to_ascii_lowercase().contains(pattern))
                .cloned()
        })
    };
    if prompt_has_any(
        prompt,
        &["店铺", "门店", "分店", "店名", "柜组", "专柜", "shop"],
    ) {
        return find_column(&[
            "shopdesc",
            "shop_name",
            "shopname",
            "storedesc",
            "store_name",
            "storename",
            "store_init",
        ]);
    }
    if prompt_has_any(prompt, &["品牌", "租户", "商户", "brand", "lease"]) {
        return find_column(&[
            "branddesc",
            "brand_name",
            "brandname",
            "leasename",
            "lease_name",
        ]);
    }
    if prompt_has_any(prompt, &["品类", "业态", "类目", "category"]) {
        return find_column(&["catgldesc", "catgmdesc", "catgsdesc", "category"]);
    }
    if prompt_has_any(prompt, &["大区", "区域", "小区", "片区", "region", "area"]) {
        return find_column(&[
            "dist_name",
            "areaname",
            "area_name",
            "region",
            "omdname",
            "parentname",
            "area",
        ]);
    }
    if prompt_has_any(prompt, &["合同", "contract"]) {
        return find_column(&["contract_no", "htbh"]);
    }
    None
}

pub(crate) fn assistant_run_database_aggregate_dimensions(
    mapping: &MySqlTableMapping,
    prompt: &str,
) -> Vec<String> {
    let mut dimensions = Vec::new();
    let wants_time = prompt_has_any(
        prompt,
        &[
            "时间", "日期", "小时", "日", "趋势", "txdate", "date", "time", "by time",
        ],
    );
    let wants_entity = prompt_has_any(
        prompt,
        &[
            "区域", "位置", "楼层", "门", "梯", "点位", "areaname", "area", "top", "排名", "排行",
            "排序", "前",
        ],
    );
    if wants_entity {
        if let Some(dimension) = assistant_run_database_prompt_entity_dimension(mapping, prompt)
            .or_else(|| assistant_run_database_entity_dimension(mapping))
        {
            push_unique_string(&mut dimensions, &dimension);
        }
    }
    if wants_time {
        if let Some(time_column) = assistant_run_database_time_column(mapping) {
            push_unique_string(&mut dimensions, &time_column);
        }
    }
    if dimensions.is_empty() {
        if let Some(title_column) = mapping.title_column.as_deref() {
            push_unique_string(&mut dimensions, title_column);
        }
    }
    dimensions.truncate(3);
    dimensions
}

pub(crate) fn assistant_run_database_aggregate_metrics(
    mapping: &MySqlTableMapping,
    prompt: &str,
) -> Vec<String> {
    assistant_run_database_explicit_metric_columns(mapping, prompt)
        .into_iter()
        .filter(|column| assistant_run_database_automatic_aggregation_allowed(column))
        .collect()
}

fn assistant_run_database_automatic_aggregation_allowed(column: &str) -> bool {
    let lower = column.trim().to_ascii_lowercase();
    !matches!(lower.as_str(), "quekou" | "xuzengxiaoshou")
        && ![
            "rate",
            "ratio",
            "percent",
            "percentage",
            "pct",
            "share",
            "score",
            "index",
        ]
        .iter()
        .any(|segment| {
            lower
                .split(|character: char| !character.is_ascii_alphanumeric())
                .any(|candidate| candidate == *segment)
        })
}

fn assistant_run_database_explicit_metric_columns(
    mapping: &MySqlTableMapping,
    prompt: &str,
) -> Vec<String> {
    let mut metrics = Vec::new();
    let wants_sales_gap = prompt_has_metric_terms(prompt, &["销售缺口", "缺口", "quekou"]);
    if wants_sales_gap {
        push_database_metric_if_present(mapping, &mut metrics, "quekou");
    }
    if !wants_sales_gap
        && prompt_has_metric_terms(
            prompt,
            &[
                "取高",
                "高分成",
                "分成线",
                "差多少",
                "还差",
                "需增",
                "机会",
                "接近",
                "就快",
            ],
        )
    {
        push_database_metric_if_present(mapping, &mut metrics, "xuzengxiaoshou");
        push_database_metric_if_present(mapping, &mut metrics, "quekou");
    }
    if !wants_sales_gap
        && prompt_has_metric_terms(prompt, &["销售", "销售额", "成交", "sale", "sales", "sold"])
    {
        push_database_metric_if_present(mapping, &mut metrics, "amttotal");
        push_database_metric_if_present(mapping, &mut metrics, "amtsold");
        push_database_metric_if_present(mapping, &mut metrics, "sale_num");
        push_database_metric_if_present(mapping, &mut metrics, "salenum");
    }
    if prompt_has_metric_terms(prompt, &["租金", "提成", "固定", "rent", "fee"]) {
        push_database_metric_if_present(mapping, &mut metrics, "tichengzujin");
        push_database_metric_if_present(mapping, &mut metrics, "yuezujin");
        push_database_metric_if_present(mapping, &mut metrics, "htzj");
    }
    if prompt_has_metric_terms(prompt, &["下行", "离开", "离场", "出场", "出口"])
        || (prompt_has_metric_terms(prompt, &["down"])
            && assistant_run_database_ascii_direction_metric_context(mapping, prompt))
    {
        push_database_metric_if_present(mapping, &mut metrics, "down");
    }
    if prompt_has_metric_terms(prompt, &["上行", "进入", "进场", "入口"])
        || (prompt_has_metric_terms(prompt, &["up"])
            && assistant_run_database_ascii_direction_metric_context(mapping, prompt))
    {
        push_database_metric_if_present(mapping, &mut metrics, "up");
    }
    if metrics.is_empty() && prompt_has_metric_terms(prompt, &["流量", "客流", "traffic"]) {
        push_database_metric_if_present(mapping, &mut metrics, "up");
        push_database_metric_if_present(mapping, &mut metrics, "down");
    }
    metrics.truncate(ASSISTANT_RUN_DATABASE_AGGREGATE_METRIC_LIMIT);
    metrics
}

fn assistant_run_database_ascii_direction_metric_context(
    mapping: &MySqlTableMapping,
    prompt: &str,
) -> bool {
    assistant_run_database_prompt_mentions_identifier(&prompt.to_ascii_lowercase(), &mapping.table)
        || prompt_has_metric_terms(
            prompt,
            &[
                "traffic", "flow", "metric", "field", "sum", "avg", "average", "max", "min", "top",
                "rank",
            ],
        )
}

fn push_database_metric_if_present(
    mapping: &MySqlTableMapping,
    metrics: &mut Vec<String>,
    column: &str,
) {
    if metrics.iter().any(|metric| metric == column) {
        return;
    }
    if assistant_run_database_mapping_columns(mapping)
        .iter()
        .any(|candidate| candidate == column)
    {
        metrics.push(column.to_string());
    }
}

pub(crate) fn assistant_run_database_aggregate_latest_time_column(
    mapping: &MySqlTableMapping,
    prompt: &str,
    metric: Option<&str>,
    aggregate_plan: &AssistantRunDatabaseAggregatePlan,
) -> Option<String> {
    if aggregate_plan.intent == "time_series" {
        return None;
    }
    let time_column = assistant_run_database_time_column(mapping)?;
    if aggregate_plan
        .dimensions
        .iter()
        .any(|dimension| dimension == &time_column)
    {
        return None;
    }
    if prompt_has_any(
        prompt,
        &[
            "趋势",
            "变化",
            "按日",
            "按天",
            "按时间",
            "历史",
            "区间",
            "同比",
            "环比",
            "trend",
        ],
    ) {
        return None;
    }
    let metric = metric.unwrap_or("").to_ascii_lowercase();
    let current_metric = metric.contains("yuezujin")
        || metric.contains("zujin")
        || metric.contains("rent")
        || metric.contains("amttotal")
        || metric.contains("ticheng")
        || metric.contains("xuzeng")
        || metric.contains("quekou")
        || metric.contains("dayamt")
        || metric.contains("salenum")
        || metric.contains("sale");
    let current_prompt = prompt_has_any(
        prompt,
        &[
            "当前",
            "现在",
            "目前",
            "本月",
            "最新",
            "经营健康度",
            "总览",
            "月租金",
            "租金",
            "取高",
            "高分成",
            "缺口",
            "销售额",
            "还差",
            "最接近",
        ],
    );
    if current_metric || current_prompt {
        return Some(time_column);
    }
    None
}

pub(crate) fn assistant_run_database_aggregate_requested(prompt: &str) -> bool {
    if assistant_run_database_schema_overview_requested(prompt) {
        return false;
    }
    let concept_or_method_question = prompt_has_any(
        prompt,
        &[
            "什么是",
            "是什么意思",
            "含义",
            "公式",
            "方法",
            "教程",
            "怎么统计",
            "如何统计",
            "怎么计算",
            "如何计算",
            "how to",
            "what is",
            "what does",
            "tutorial",
            "method",
        ],
    );
    let explicit_result_selector = prompt_has_any(
        prompt,
        &[
            "多少", "哪些", "哪个", "最高", "最低", "最大", "最小", "排行", "排名", "前几", "top ",
            "highest", "lowest",
        ],
    );
    if concept_or_method_question && !explicit_result_selector {
        return false;
    }
    prompt_has_any(
        prompt,
        &[
            "统计",
            "汇总",
            "合计",
            "排序",
            "排名",
            "排行",
            "前",
            "最高",
            "最大",
            "最低",
            "最小",
            "平均",
            "趋势",
            "维度",
            "占比",
            "比例",
            "分布",
            "报表",
            "表格",
            "上行",
            "下行",
            "流量",
            "区域",
            "楼层",
            "哪些",
            "哪个",
            "多少",
            "店铺",
            "门店",
            "分店",
            "品牌",
            "品类",
            "业态",
            "取高",
            "高分成",
            "缺口",
            "机会",
            "助推",
            "低活跃",
            "风险",
            "预警",
            "销售额",
            "租金",
            "客流",
            "坪效",
            "租售比",
            "经营",
            "健康度",
            "总览",
            "数据",
        ],
    ) || prompt_has_metric_terms(
        prompt,
        &["top", "rank", "sum", "avg", "max", "min", "up", "down"],
    )
}

pub(crate) fn assistant_run_database_count_aggregation_requested(prompt: &str) -> bool {
    prompt_has_any(
        prompt,
        &[
            "记录数量",
            "记录数",
            "数据量",
            "条数",
            "多少条",
            "多少个记录",
            "count(*)",
            "count records",
            "record count",
            "row count",
        ],
    )
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

pub(crate) fn assistant_run_database_schema_context_requested(prompt: &str) -> bool {
    if assistant_run_database_schema_overview_requested(prompt)
        || assistant_run_database_aggregate_requested(prompt)
        || prompt_has_any(
            prompt,
            &[
                "数据库",
                "数据表",
                "表结构",
                "字段",
                "指标",
                "维度",
                "口径",
                "数据源",
                "数据集",
                "内容",
                "是什么",
                "有什么",
                "分析",
            ],
        )
    {
        return true;
    }
    let normalized = prompt.to_ascii_lowercase();
    [
        "schema",
        "field",
        "metric",
        "dimension",
        "source",
        "dataset",
    ]
    .iter()
    .any(|token| contains_ascii_token(&normalized, token))
}

pub(crate) fn assistant_run_database_prompt_wants_report(prompt: &str) -> bool {
    prompt_has_any(
        prompt,
        &[
            "报表",
            "报告",
            "看板",
            "仪表盘",
            "图文",
            "静态页",
            "html",
            "dashboard",
            "report",
            "visual",
        ],
    )
}

pub(crate) fn assistant_run_database_aggregation(prompt: &str, has_metric: bool) -> String {
    if !has_metric {
        return "count".to_string();
    }
    if prompt_has_any(prompt, &["平均"]) || prompt_has_metric_terms(prompt, &["avg", "average"]) {
        "avg".to_string()
    } else if prompt_has_any(prompt, &["最低", "最小"]) || prompt_has_metric_terms(prompt, &["min"])
    {
        "min".to_string()
    } else if prompt_has_any(prompt, &["单点最大", "最大值"])
        || prompt_has_metric_terms(prompt, &["max"])
    {
        "max".to_string()
    } else {
        "sum".to_string()
    }
}

pub(crate) fn assistant_run_database_aggregate_order_direction(
    prompt: &str,
    _metric: Option<&str>,
) -> String {
    if prompt_has_any(prompt, &["最低", "最小", "升序"])
        || prompt_has_metric_terms(prompt, &["min"])
    {
        return "asc".to_string();
    }
    "desc".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(table: &str) -> MySqlTableMapping {
        MySqlTableMapping {
            table: table.to_string(),
            object_type: "document".to_string(),
            id_column: "id".to_string(),
            id_columns: Vec::new(),
            title_column: Some("name".to_string()),
            content_columns: Vec::new(),
            content_type: "text/markdown".to_string(),
            updated_at_column: None,
            version_column: None,
            revision_strategy: Default::default(),
            metadata_columns: Vec::new(),
        }
    }

    fn config_with_tables(tables: &[&str]) -> MySqlSourceConfig {
        MySqlSourceConfig {
            connection_env: "TEST_DATABASE_URL".to_string(),
            database: "test".to_string(),
            timeout_ms: 10_000,
            row_limit: 1_000,
            page_size: 1_000,
            default_dataset_id: None,
            tables: tables.iter().map(|table| mapping(table)).collect(),
        }
    }

    fn single_column_mapping() -> MySqlTableMapping {
        MySqlTableMapping {
            table: "s".to_string(),
            object_type: "document".to_string(),
            id_column: "a".to_string(),
            id_columns: vec!["a".to_string()],
            title_column: None,
            content_columns: vec!["a".to_string()],
            content_type: "text/markdown".to_string(),
            updated_at_column: None,
            version_column: None,
            revision_strategy: Default::default(),
            metadata_columns: Vec::new(),
        }
    }

    fn contract_warning_mapping() -> MySqlTableMapping {
        MySqlTableMapping {
            table: "bi_contract_warning".to_string(),
            object_type: "document".to_string(),
            id_column: "parentcode".to_string(),
            id_columns: vec!["parentcode".to_string(), "storecode".to_string()],
            title_column: Some("shopdesc".to_string()),
            content_columns: vec![
                "shopdesc".to_string(),
                "quekou".to_string(),
                "xuzengxiaoshou".to_string(),
            ],
            content_type: "text/markdown".to_string(),
            updated_at_column: Some("txdate".to_string()),
            version_column: None,
            revision_strategy: Default::default(),
            metadata_columns: vec!["quekou".to_string(), "xuzengxiaoshou".to_string()],
        }
    }

    fn traffic_mapping() -> MySqlTableMapping {
        MySqlTableMapping {
            table: "traffic_area".to_string(),
            object_type: "traffic".to_string(),
            id_column: "storecode".to_string(),
            id_columns: vec!["storecode".to_string()],
            title_column: Some("areaname".to_string()),
            content_columns: vec!["areaname".to_string()],
            content_type: "text/markdown".to_string(),
            updated_at_column: Some("txdate".to_string()),
            version_column: None,
            revision_strategy: Default::default(),
            metadata_columns: vec![
                "txdate".to_string(),
                "areatype".to_string(),
                "up".to_string(),
                "down".to_string(),
            ],
        }
    }

    #[test]
    fn database_prompt_support_selects_prompt_table_or_limited_fallback_tables() {
        let config = config_with_tables(&[
            "traffic_area",
            "store_contract",
            "sales_daily",
            "inventory",
            "extra",
        ]);
        assert_eq!(
            assistant_run_database_mapping_for_prompt(&config, "看 store_contract 的取高机会")
                .map(|mapping| mapping.table.as_str()),
            Some("store_contract")
        );
        assert_eq!(
            assistant_run_database_mapping_for_prompt(&config, "看整体经营")
                .map(|mapping| mapping.table.as_str()),
            None
        );
        assert_eq!(
            assistant_run_database_schema_mappings_for_prompt(&config, "先看 sales_daily 字段")
                .into_iter()
                .map(|mapping| mapping.table.as_str())
                .collect::<Vec<_>>(),
            vec!["sales_daily"]
        );
        assert_eq!(
            assistant_run_database_schema_mappings_for_prompt(&config, "有哪些表")
                .into_iter()
                .map(|mapping| mapping.table.as_str())
                .collect::<Vec<_>>(),
            vec!["traffic_area", "store_contract", "sales_daily", "inventory"]
        );
        for prompt in [
            "分析一下现金流折现是什么意思",
            "为什么这个销售指标会波动",
            "what is geometric mean",
            "resource allocation",
            "what is database schema",
            "how to design a database schema",
            "什么是数据库结构",
            "database schema design principles",
            "数据库结构设计原则",
            "schema best practices",
        ] {
            assert!(
                assistant_run_database_schema_mappings_for_prompt(&config, prompt).is_empty(),
                "ordinary concept question must not receive arbitrary schema fallback: {prompt}"
            );
        }
    }

    #[test]
    fn database_prompt_support_does_not_match_single_letter_table_inside_field_name() {
        let config = MySqlSourceConfig {
            connection_env: "TEST_DATABASE_URL".to_string(),
            database: "test".to_string(),
            timeout_ms: 10_000,
            row_limit: 1_000,
            page_size: 1_000,
            default_dataset_id: None,
            tables: vec![single_column_mapping(), contract_warning_mapping()],
        };

        assert_eq!(
            assistant_run_database_mapping_for_prompt(
                &config,
                "解释销售缺口最大的门店，同时引用报告里的风险描述。"
            )
            .map(|mapping| mapping.table.as_str()),
            Some("bi_contract_warning")
        );
        assert_eq!(
            assistant_run_database_explicit_metric_columns(
                &contract_warning_mapping(),
                "解释销售缺口最大的门店"
            ),
            vec!["quekou".to_string()]
        );
        assert!(assistant_run_database_aggregate_metrics(
            &contract_warning_mapping(),
            "解释销售缺口最大的门店"
        )
        .is_empty());
        assert_eq!(
            assistant_run_database_mapping_for_prompt(&config, "解释一下什么是现金流折现")
                .map(|mapping| mapping.table.as_str()),
            None
        );
        for prompt in [
            "what is a database metric",
            "what's a database metric",
            "what is a database id",
            "database date field",
        ] {
            assert!(
                assistant_run_database_mapping_for_prompt(&config, prompt).is_none(),
                "generic identity, time, or one-character words must not select a mapping: {prompt}"
            );
            assert!(assistant_run_database_schema_mappings_for_prompt(&config, prompt).is_empty());
        }
    }

    #[test]
    fn database_prompt_support_fails_closed_on_ambiguous_mapping_or_missing_metric() {
        let mut archive_mapping = contract_warning_mapping();
        archive_mapping.table = "bi_contract_warning_archive".to_string();
        let ambiguous = MySqlSourceConfig {
            connection_env: "TEST_DATABASE_URL".to_string(),
            database: "test".to_string(),
            timeout_ms: 10_000,
            row_limit: 1_000,
            page_size: 1_000,
            default_dataset_id: None,
            tables: vec![contract_warning_mapping(), archive_mapping],
        };

        assert!(
            assistant_run_database_mapping_for_prompt(&ambiguous, "解释销售缺口最大的门店")
                .is_none()
        );
        assert!(assistant_run_database_aggregate_metrics(
            &contract_warning_mapping(),
            "bi_contract_warning 里哪些门店经营风险最高"
        )
        .is_empty());
    }

    #[test]
    fn database_prompt_support_builds_report_dimension_plans() {
        let plans = assistant_run_database_aggregate_dimension_plans(
            &traffic_mapping(),
            "生成一页图文 HTML 流量分析报表，展示区域 Top5、趋势和类型对比",
        );

        assert!(plans.iter().any(|plan| {
            plan.role == "ranking" && plan.dimensions == vec!["areaname".to_string()]
        }));
        assert!(plans
            .iter()
            .any(|plan| plan.role == "trend" && plan.dimensions == vec!["txdate".to_string()]));
        assert!(plans.iter().any(|plan| {
            plan.role == "comparison" && plan.dimensions == vec!["areatype".to_string()]
        }));
    }

    #[test]
    fn database_prompt_support_keeps_core_aggregate_intents() {
        assert!(assistant_run_database_aggregate_requested("经营健康度总览"));
        assert!(assistant_run_database_schema_context_requested(
            "这里有什么字段"
        ));
        for prompt in [
            "有哪些表",
            "这个数据库有哪些表和字段",
            "这个数据源有哪些字段和指标",
            "我这里有哪些数据可以用么",
            "查看数据库结构",
            "schema overview",
        ] {
            assert!(
                assistant_run_database_schema_overview_requested(prompt),
                "explicit schema overview should be recognized: {prompt}"
            );
            assert!(assistant_run_database_schema_context_requested(prompt));
            assert!(
                !assistant_run_database_aggregate_requested(prompt),
                "pure schema overview must not be diverted into aggregate planning: {prompt}"
            );
        }
        assert!(assistant_run_database_aggregate_requested(
            "哪些门店风险最高"
        ));
        assert!(!assistant_run_database_schema_context_requested(
            "resource allocation"
        ));
        assert!(!assistant_run_database_schema_context_requested(
            "what is geometric mean"
        ));
        for prompt in [
            "what is database schema",
            "how to design a database schema",
            "什么是数据库结构",
            "what is a schema overview",
            "how to list tables",
            "schema overview best practices",
            "怎么查看数据库结构",
        ] {
            assert!(
                !assistant_run_database_schema_overview_requested(prompt),
                "schema concept/tutorial question must not receive configured-table overview: {prompt}"
            );
        }
        for prompt in ["请查看数据库结构", "请列出表", "schema overview"] {
            assert!(
                assistant_run_database_schema_overview_requested(prompt),
                "explicit schema listing request should remain supported: {prompt}"
            );
        }
        for prompt in ["sales topic", "sales upgrade", "sales admin guide"] {
            assert!(
                !assistant_run_database_aggregate_requested(prompt),
                "ASCII aggregate fragments must require token boundaries: {prompt}"
            );
        }
        for prompt in [
            "销售额平均是什么意思",
            "销售额怎么统计",
            "销售额如何计算",
            "how to sum sales",
        ] {
            assert!(
                !assistant_run_database_aggregate_requested(prompt),
                "concept or method question must not run an aggregate: {prompt}"
            );
        }
        for prompt in ["what is up", "scroll down", "sales are down"] {
            assert!(
                assistant_run_database_explicit_metric_columns(&traffic_mapping(), prompt)
                    .is_empty(),
                "ordinary English direction words must not select traffic metrics: {prompt}"
            );
        }
        assert_eq!(
            assistant_run_database_explicit_metric_columns(
                &traffic_mapping(),
                "sum traffic down by area"
            ),
            vec!["down".to_string()]
        );
        assert!(assistant_run_database_aggregate_requested("top sales"));
        assert!(assistant_run_database_aggregate_requested("sum sales"));
        assert_eq!(
            assistant_run_database_aggregation("sales admin guide", true),
            "sum"
        );
        assert_eq!(assistant_run_database_aggregation("min sales", true), "min");
        assert!(assistant_run_database_prompt_wants_report(
            "生成图文 HTML 报表"
        ));
        assert_eq!(
            assistant_run_database_aggregation("按区域统计记录数量", false),
            "count"
        );
        assert!(assistant_run_database_count_aggregation_requested(
            "按区域统计记录数量"
        ));
        assert!(!assistant_run_database_count_aggregation_requested(
            "解释销售缺口最大的门店"
        ));
        assert_eq!(
            assistant_run_database_aggregate_order_direction(
                "取高机会最大的店铺是哪几个",
                Some("xuzengxiaoshou")
            ),
            "desc"
        );
    }
}
