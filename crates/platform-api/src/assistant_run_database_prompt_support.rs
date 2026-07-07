use crate::{
    assistant_run_database_field_support::{
        assistant_run_database_category_dimension, assistant_run_database_mapping_columns,
        assistant_run_database_metric_columns, assistant_run_database_time_column,
    },
    prompt_match_support::{prompt_has_any, prompt_has_metric_terms},
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
    let normalized = prompt.to_ascii_lowercase();
    config
        .tables
        .iter()
        .find(|mapping| normalized.contains(&mapping.table.to_ascii_lowercase()))
        .or_else(|| config.tables.first())
}

pub(crate) fn assistant_run_database_schema_mappings_for_prompt<'a>(
    config: &'a MySqlSourceConfig,
    prompt: &str,
) -> Vec<&'a MySqlTableMapping> {
    let normalized = prompt.to_ascii_lowercase();
    let mut mappings = config
        .tables
        .iter()
        .filter(|mapping| normalized.contains(&mapping.table.to_ascii_lowercase()))
        .take(ASSISTANT_RUN_DATABASE_SCHEMA_TABLE_LIMIT)
        .collect::<Vec<_>>();
    if mappings.is_empty() {
        mappings = config
            .tables
            .iter()
            .take(ASSISTANT_RUN_DATABASE_SCHEMA_TABLE_LIMIT)
            .collect();
    }
    mappings
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
    let mut metrics = Vec::new();
    if prompt_has_metric_terms(
        prompt,
        &[
            "取高",
            "高分成",
            "分成线",
            "缺口",
            "差多少",
            "还差",
            "需增",
            "机会",
            "接近",
            "就快",
        ],
    ) {
        push_database_metric_if_present(mapping, &mut metrics, "xuzengxiaoshou");
        push_database_metric_if_present(mapping, &mut metrics, "quekou");
    }
    if prompt_has_metric_terms(prompt, &["销售", "销售额", "成交", "sale", "sales", "sold"])
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
    if prompt_has_metric_terms(prompt, &["down", "下行", "离开", "离场", "出场", "出口"])
    {
        push_database_metric_if_present(mapping, &mut metrics, "down");
    }
    if prompt_has_metric_terms(prompt, &["up", "上行", "进入", "进场", "入口"]) {
        push_database_metric_if_present(mapping, &mut metrics, "up");
    }
    if metrics.is_empty() && prompt_has_metric_terms(prompt, &["流量", "客流", "traffic"]) {
        push_database_metric_if_present(mapping, &mut metrics, "up");
        push_database_metric_if_present(mapping, &mut metrics, "down");
    }
    if metrics.is_empty() {
        for column in assistant_run_database_metric_columns(mapping) {
            push_unique_string(&mut metrics, &column);
            if metrics.len() >= ASSISTANT_RUN_DATABASE_AGGREGATE_METRIC_LIMIT {
                break;
            }
        }
    }
    metrics.truncate(ASSISTANT_RUN_DATABASE_AGGREGATE_METRIC_LIMIT);
    metrics
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
            "top",
            "rank",
            "sum",
            "avg",
            "max",
            "min",
            "up",
            "down",
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
    assistant_run_database_aggregate_requested(prompt)
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
                "schema",
                "field",
                "metric",
                "dimension",
                "source",
                "dataset",
            ],
        )
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
    if prompt_has_any(prompt, &["平均", "avg", "average"]) {
        "avg".to_string()
    } else if prompt_has_any(prompt, &["最低", "最小", "min"]) {
        "min".to_string()
    } else if prompt_has_any(prompt, &["单点最大", "最大值", "max"]) {
        "max".to_string()
    } else {
        "sum".to_string()
    }
}

pub(crate) fn assistant_run_database_aggregate_order_direction(
    prompt: &str,
    metric: Option<&str>,
) -> String {
    if prompt_has_any(prompt, &["最低", "最小", "min", "升序"]) {
        return "asc".to_string();
    }
    let metric = metric.unwrap_or("").to_ascii_lowercase();
    let gap_metric = metric.contains("xuzeng")
        || metric.contains("quekou")
        || metric.contains("gap")
        || metric.contains("shortfall");
    if gap_metric
        && prompt_has_any(
            prompt,
            &[
                "取高机会",
                "机会最大",
                "最接近",
                "接近",
                "就快",
                "差多少",
                "还差",
                "高分成线",
            ],
        )
        && !prompt_has_any(prompt, &["缺口最大", "最大缺口", "差距最大"])
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
            Some("traffic_area")
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
        assert!(assistant_run_database_prompt_wants_report(
            "生成图文 HTML 报表"
        ));
        assert_eq!(
            assistant_run_database_aggregation("按区域统计记录数量", false),
            "count"
        );
        assert_eq!(
            assistant_run_database_aggregate_order_direction(
                "取高机会最大的店铺是哪几个",
                Some("xuzengxiaoshou")
            ),
            "asc"
        );
    }
}
