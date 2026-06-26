use crate::assistant_run_resume_prompt_support::{
    prompt_requests_resume_certificate_ranking, prompt_requests_resume_company_ranking,
    prompt_requests_resume_education_ranking, prompt_requests_resume_experience_statistics,
    prompt_requests_resume_location_ranking, prompt_requests_resume_position_ranking,
    prompt_requests_resume_project_ranking, prompt_requests_resume_skill_ranking,
};
use crate::prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any};
use crate::{assistant_run_entity_scan_answer_dimension, AssistantRunEntityScanAnswerDimension};

pub(crate) fn prompt_requests_spreadsheet_row_level_analysis(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    let has_row_context = prompt_contains_any(
        prompt,
        &[
            "考勤", "缺勤", "出勤", "工时", "打卡", "上班", "下班", "迟到", "早退", "请假", "排班",
            "班次", "每天", "日期", "记录", "明细",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "attendance",
            "absence",
            "absent",
            "workhour",
            "workhours",
            "hours",
            "clock",
            "checkin",
            "checkout",
            "date",
            "daily",
            "row",
            "rows",
            "record",
            "records",
        ],
    );
    let has_row_question = prompt_contains_any(
        prompt,
        &[
            "最长",
            "最短",
            "最早",
            "最晚",
            "多少",
            "谁",
            "哪个",
            "哪些",
            "列出",
            "表格",
            "排序",
            "长短",
            "有没",
            "有没有",
            "人",
            "人员",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "longest", "shortest", "earliest", "latest", "who", "which", "list", "table", "sort",
            "rank",
        ],
    );
    has_row_context && has_row_question
}

pub(crate) fn prompt_requests_document_entity_scan(prompt: &str) -> bool {
    if assistant_run_entity_scan_answer_dimension(prompt).is_some() {
        return true;
    }
    if prompt_requests_resume_company_entity_scan(prompt) {
        return true;
    }
    if prompt_requests_document_dimension_aggregate(prompt)
        || prompt_requests_document_scoped_business_aggregate(prompt)
    {
        return true;
    }

    let lower_prompt = prompt.to_ascii_lowercase();
    let has_entity_signal = prompt_contains_any(
        prompt,
        &[
            "公司名",
            "公司",
            "企业",
            "组织",
            "机构",
            "单位",
            "雇主",
            "人员",
            "姓名",
            "联系人",
            "候选人",
            "岗位",
            "职位",
            "职务",
            "角色",
            "技能",
            "技术栈",
            "能力",
            "项目",
            "产品",
            "系统",
            "平台",
            "地点",
            "位置",
            "点位",
            "楼层",
            "区域",
            "入口",
            "出口",
            "出入口",
            "门禁",
            "梯控",
            "电梯",
            "直梯",
            "扶梯",
            "手扶梯",
            "观光电梯",
            "城市",
            "地区",
            "地址",
            "学校",
            "院校",
            "大学",
            "学院",
            "学历",
            "学位",
            "证书",
            "认证",
            "资质",
            "实体",
            "名词",
            "专有名词",
            "关键词",
            "关键字",
            "术语",
            "标签",
            "分词",
            "门店",
            "分店",
            "店铺",
            "品牌",
            "品类",
            "业态",
            "类别",
            "经营",
            "取高",
            "高分成",
            "缺口",
            "低活跃",
            "风险",
            "机会",
            "销售额",
            "租金",
            "客流",
            "坪效",
            "租售比",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "entity",
            "entities",
            "noun",
            "nouns",
            "keyword",
            "keywords",
            "term",
            "terms",
            "organization",
            "company",
            "companies",
            "employer",
            "person",
            "people",
            "name",
            "names",
            "candidate",
            "candidates",
            "position",
            "positions",
            "role",
            "roles",
            "title",
            "titles",
            "skill",
            "skills",
            "project",
            "projects",
            "product",
            "products",
            "system",
            "systems",
            "platform",
            "platforms",
            "location",
            "locations",
            "point",
            "points",
            "site",
            "sites",
            "floor",
            "floors",
            "area",
            "areas",
            "entrance",
            "entrances",
            "exit",
            "exits",
            "access",
            "elevator",
            "elevators",
            "city",
            "cities",
            "school",
            "schools",
            "university",
            "universities",
            "college",
            "colleges",
            "degree",
            "degrees",
            "education",
            "certificate",
            "certificates",
            "certification",
            "store",
            "stores",
            "shop",
            "shops",
            "brand",
            "brands",
            "category",
            "categories",
            "risk",
            "risks",
            "opportunity",
            "opportunities",
            "sales",
            "rent",
            "traffic",
        ],
    ) || lower_prompt.contains("tech stack");
    let has_coverage_signal = prompt_contains_any(
        prompt,
        &[
            "多少",
            "几个",
            "哪些",
            "有什么",
            "有哪些",
            "列出",
            "统计",
            "汇总",
            "分布",
            "全部",
            "所有",
            "提到",
            "出现",
            "抽取",
            "提取",
            "识别",
            "扫描",
            "归纳",
            "整理",
            "清单",
            "出表",
            "表格",
            "成表",
            "去重",
            "频次",
            "频率",
            "分词",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "count",
            "list",
            "all",
            "extract",
            "scan",
            "identify",
            "summarize",
            "summary",
            "inventory",
            "dedupe",
            "frequency",
            "frequencies",
            "top",
            "which",
        ],
    ) || lower_prompt.contains("how many")
        || lower_prompt.contains("what are");

    has_entity_signal && (has_coverage_signal || prompt.contains("公司名"))
}

pub(crate) fn prompt_requests_deterministic_aggregate_supply(prompt: &str) -> bool {
    prompt_requests_spreadsheet_row_level_analysis(prompt)
        || prompt_requests_resume_company_entity_scan(prompt)
        || prompt_requests_resume_skill_ranking(prompt)
        || prompt_requests_resume_project_ranking(prompt)
        || prompt_requests_resume_position_ranking(prompt)
        || prompt_requests_resume_location_ranking(prompt)
        || prompt_requests_resume_company_ranking(prompt)
        || prompt_requests_resume_education_ranking(prompt)
        || prompt_requests_resume_certificate_ranking(prompt)
        || prompt_requests_resume_experience_statistics(prompt)
        || prompt_requests_document_dimension_aggregate(prompt)
        || prompt_requests_business_metric_aggregate(prompt)
}

pub(crate) fn assistant_run_fact_snapshot_can_replace_dataset_entity_scan(prompt: &str) -> bool {
    if assistant_run_prompt_requests_point_list_table(prompt) {
        return false;
    }
    matches!(
        assistant_run_entity_scan_answer_dimension(prompt),
        Some(
            AssistantRunEntityScanAnswerDimension::Company
                | AssistantRunEntityScanAnswerDimension::Skill
                | AssistantRunEntityScanAnswerDimension::Project
                | AssistantRunEntityScanAnswerDimension::Position
                | AssistantRunEntityScanAnswerDimension::Person
                | AssistantRunEntityScanAnswerDimension::Location
                | AssistantRunEntityScanAnswerDimension::Certificate
                | AssistantRunEntityScanAnswerDimension::Keyword
                | AssistantRunEntityScanAnswerDimension::Year
                | AssistantRunEntityScanAnswerDimension::Section
        )
    )
}

pub(crate) fn assistant_run_prompt_requests_point_list_table(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    let has_point_subject = prompt_contains_any(
        prompt,
        &["智能梯控", "电梯", "扶梯", "梯控", "点位", "楼层", "位置"],
    ) || ascii_prompt_contains_any(
        &lower,
        &["elevator", "escalator", "point", "floor", "location"],
    );
    let has_table_or_list =
        prompt_contains_any(prompt, &["有哪些", "出表", "表格", "列出", "按楼层"])
            || ascii_prompt_contains_any(&lower, &["list", "table"]);
    has_point_subject && has_table_or_list
}

fn prompt_requests_document_dimension_aggregate(prompt: &str) -> bool {
    if assistant_run_entity_scan_answer_dimension(prompt).is_some() {
        return true;
    }
    let lower_prompt = prompt.to_ascii_lowercase();
    let has_dimension = prompt_contains_any(
        prompt,
        &[
            "公司",
            "企业",
            "组织",
            "机构",
            "人员",
            "候选人",
            "岗位",
            "职位",
            "技能",
            "项目",
            "产品",
            "系统",
            "平台",
            "地点",
            "位置",
            "区域",
            "学校",
            "学历",
            "证书",
            "关键词",
            "名词",
            "年份",
            "章节",
            "段落",
            "表格",
            "品类",
            "类别",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "company",
            "organization",
            "person",
            "candidate",
            "role",
            "skill",
            "project",
            "product",
            "system",
            "location",
            "region",
            "school",
            "degree",
            "certificate",
            "keyword",
            "term",
            "year",
            "section",
            "paragraph",
            "table",
            "category",
        ],
    );
    has_dimension && prompt_requests_aggregate_operation(prompt)
}

fn prompt_requests_business_metric_aggregate(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    let has_business_dimension = prompt_contains_any(
        prompt,
        &[
            "门店",
            "分店",
            "店铺",
            "品牌",
            "品牌店",
            "品类",
            "业态",
            "区域",
            "分区",
            "经营",
            "健康度",
            "取高",
            "高分成",
            "缺口",
            "助推",
            "低活跃",
            "风险",
            "机会",
            "销售",
            "销售额",
            "租金",
            "客流",
            "坪效",
            "租售比",
            "预警",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "store",
            "stores",
            "shop",
            "shops",
            "brand",
            "brands",
            "category",
            "categories",
            "risk",
            "risks",
            "opportunity",
            "opportunities",
            "sales",
            "rent",
            "traffic",
            "warning",
        ],
    );
    has_business_dimension && prompt_requests_aggregate_operation(prompt)
}

fn prompt_requests_document_scoped_business_aggregate(prompt: &str) -> bool {
    if !prompt_requests_business_metric_aggregate(prompt) {
        return false;
    }
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_contains_any(
        prompt,
        &[
            "文档",
            "资料",
            "知识库",
            "文件",
            "附件",
            "表格",
            "合同",
            "手册",
            "解析",
            "抽取",
            "入库",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "document",
            "documents",
            "file",
            "files",
            "attachment",
            "spreadsheet",
            "contract",
            "extract",
            "parse",
        ],
    )
}

fn prompt_requests_aggregate_operation(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_contains_any(
        prompt,
        &[
            "多少",
            "几个",
            "数量",
            "总数",
            "统计",
            "汇总",
            "合计",
            "占比",
            "比例",
            "分布",
            "排行",
            "排名",
            "排序",
            "前",
            "最高",
            "最低",
            "最大",
            "最小",
            "平均",
            "趋势",
            "哪些",
            "哪个",
            "列出",
            "清单",
            "明细",
            "出表",
            "表格",
            "识别",
            "预警",
            "看一下",
            "看看",
            "分析",
            "总览",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "count",
            "total",
            "statistic",
            "statistics",
            "summary",
            "aggregate",
            "sum",
            "ratio",
            "share",
            "distribution",
            "rank",
            "sort",
            "top",
            "max",
            "min",
            "avg",
            "trend",
            "which",
            "list",
            "table",
            "identify",
            "analyze",
            "overview",
        ],
    ) || lower_prompt.contains("how many")
}

pub(crate) fn prompt_requests_resume_company_entity_scan(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    let has_resume_signal = ["简历", "履历", "候选人", "求职", "招聘", "人才", "面试"]
        .iter()
        .any(|hint| prompt.contains(hint))
        || ["resume", "cv", "candidate", "recruit"]
            .iter()
            .any(|hint| lower_prompt.contains(hint));
    let has_company_signal = [
        "公司名",
        "公司",
        "企业",
        "雇主",
        "任职",
        "就职",
        "工作经历",
        "经历",
    ]
    .iter()
    .any(|hint| prompt.contains(hint))
        || ["company", "employer"]
            .iter()
            .any(|hint| lower_prompt.contains(hint));
    let has_coverage_signal = [
        "多少", "几个", "哪些", "列出", "统计", "汇总", "分布", "全部", "所有", "提到",
    ]
    .iter()
    .any(|hint| prompt.contains(hint))
        || ["count", "list", "all"]
            .iter()
            .any(|hint| lower_prompt.contains(hint));

    has_resume_signal && has_company_signal && (has_coverage_signal || prompt.contains("公司名"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_request_support_detects_row_level_spreadsheet_questions() {
        assert!(prompt_requests_spreadsheet_row_level_analysis(
            "这份考勤表查一下缺勤，工时最长和最短分别是谁"
        ));
        assert!(!prompt_requests_spreadsheet_row_level_analysis(
            "帮我写一段岗位技能介绍文案"
        ));
    }

    #[test]
    fn prompt_request_support_detects_document_entity_scan_domains() {
        assert!(prompt_requests_document_entity_scan(
            "智能梯控/电梯点位有哪些？请按楼层和位置出表。"
        ));
        assert!(prompt_requests_document_entity_scan(
            "这份资料里的品牌店铺客户明细按门店汇总"
        ));
        assert!(!prompt_requests_document_entity_scan("经营健康度总览"));
    }

    #[test]
    fn prompt_request_support_detects_deterministic_aggregate_domains() {
        assert!(prompt_requests_deterministic_aggregate_supply(
            "简历公司名统计一下，列出覆盖文档数"
        ));
        assert!(prompt_requests_deterministic_aggregate_supply(
            "新百风险/取高/低活跃统计，哪些品牌门店需要助推"
        ));
        assert!(!prompt_requests_deterministic_aggregate_supply(
            "帮我写一段岗位技能介绍文案"
        ));
    }

    #[test]
    fn prompt_request_support_fact_snapshot_replaces_only_supported_global_scan_prompts() {
        assert!(assistant_run_fact_snapshot_can_replace_dataset_entity_scan(
            "简历库里一共提到了多少个公司名？"
        ));
        assert!(assistant_run_fact_snapshot_can_replace_dataset_entity_scan(
            "按技能出现频次排序出表"
        ));
        assert!(!assistant_run_fact_snapshot_can_replace_dataset_entity_scan("按年龄排序出表"));
        assert!(!assistant_run_fact_snapshot_can_replace_dataset_entity_scan("按学历汇总候选人"));
        assert!(
            !assistant_run_fact_snapshot_can_replace_dataset_entity_scan(
                "智能梯控/电梯点位有哪些？请按楼层和位置出表。"
            )
        );
    }

    #[test]
    fn prompt_request_support_detects_point_list_table_prompts() {
        assert!(assistant_run_prompt_requests_point_list_table(
            "智能梯控/电梯点位有哪些？请按楼层和位置出表。"
        ));
        assert!(assistant_run_prompt_requests_point_list_table(
            "List elevator points by floor in a table"
        ));
        assert!(!assistant_run_prompt_requests_point_list_table(
            "帮我总结电梯安全注意事项"
        ));
        assert!(!assistant_run_prompt_requests_point_list_table(
            "按技能出现频次排序出表"
        ));
    }
}
