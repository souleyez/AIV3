use crate::{ascii_prompt_contains_any, prompt_contains_any};

pub(crate) fn prompt_requests_skill_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["技能", "技术栈", "能力", "专长"],
        &["skill", "skills", "tech", "stack"],
    )
}

pub(crate) fn prompt_requests_project_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["项目", "项目经验", "产品"],
        &["project", "projects"],
    )
}

pub(crate) fn prompt_requests_position_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["岗位", "职位", "职务", "角色"],
        &["position", "positions", "role", "roles", "title", "titles"],
    )
}

pub(crate) fn prompt_requests_person_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["人员", "姓名", "候选人", "联系人"],
        &[
            "person",
            "people",
            "name",
            "names",
            "candidate",
            "candidates",
        ],
    )
}

pub(crate) fn prompt_requests_location_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["地点", "城市", "地区", "所在地", "工作地点"],
        &[
            "location",
            "locations",
            "city",
            "cities",
            "region",
            "regions",
        ],
    )
}

pub(crate) fn prompt_requests_school_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &[
            "学校",
            "院校",
            "毕业院校",
            "大学",
            "学院",
            "教育经历",
            "教育背景",
        ],
        &[
            "school",
            "schools",
            "university",
            "universities",
            "college",
            "colleges",
        ],
    )
}

pub(crate) fn prompt_requests_degree_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &[
            "学历",
            "学位",
            "本科",
            "硕士",
            "博士",
            "大专",
            "专科",
            "研究生",
        ],
        &[
            "degree",
            "degrees",
            "education",
            "bachelor",
            "master",
            "phd",
            "doctorate",
        ],
    )
}

pub(crate) fn prompt_requests_certificate_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["证书", "认证", "资质", "资格证", "职业资格"],
        &[
            "certificate",
            "certificates",
            "certification",
            "certifications",
            "credential",
        ],
    )
}

pub(crate) fn prompt_requests_keyword_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["名词", "关键词", "关键字", "术语", "标签", "分词", "主题词"],
        &[
            "noun", "nouns", "keyword", "keywords", "term", "terms", "tag", "tags",
        ],
    )
}

pub(crate) fn prompt_requests_year_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["年份", "年度", "日期", "时间线"],
        &["year", "years", "date", "dates", "timeline"],
    )
}

pub(crate) fn prompt_requests_section_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["标题", "章节", "小节", "目录", "大纲", "文档结构", "层级"],
        &[
            "heading",
            "headings",
            "section",
            "sections",
            "outline",
            "toc",
            "structure",
        ],
    )
}

pub(crate) fn prompt_requests_paragraph_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["段落", "分段", "正文段", "段落结构"],
        &["paragraph", "paragraphs", "segment", "segments"],
    )
}

pub(crate) fn prompt_requests_table_statistics(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    let has_table_signal =
        prompt_contains_any(
            prompt,
            &["表格", "数据表", "明细表", "表格结构", "表格信号"],
        ) || ascii_prompt_contains_any(&lower_prompt, &["table", "tables", "tabular"]);
    let has_doc_signal = prompt_contains_any(
        prompt,
        &[
            "文档",
            "资料",
            "知识库",
            "解析",
            "抽取",
            "提取",
            "识别",
            "扫描",
            "有哪些",
            "哪些",
            "列出",
            "统计",
            "汇总",
            "结构",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "document",
            "documents",
            "extract",
            "scan",
            "identify",
            "list",
            "structure",
        ],
    );
    has_table_signal && has_doc_signal
}

pub(crate) fn prompt_requests_age_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["年龄", "岁数", "出生"],
        &["age", "ages", "birth"],
    )
}

pub(crate) fn prompt_requests_gender_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(prompt, &["性别", "男女", "男", "女"], &["gender", "sex"])
}

pub(crate) fn prompt_requests_time_statistics(prompt: &str) -> bool {
    prompt_requests_dimension_statistics(
        prompt,
        &["时间", "年份", "年限", "工作年限", "最近", "最早"],
        &["time", "year", "years", "timeline", "recent"],
    )
}

pub(crate) fn prompt_requests_resume_time_statistics(prompt: &str) -> bool {
    prompt_has_resume_signal(prompt) && prompt_requests_time_statistics(prompt)
}

pub(crate) fn prompt_has_resume_signal(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    ["简历", "履历", "候选人", "求职", "招聘", "人才", "面试"]
        .iter()
        .any(|hint| prompt.contains(hint))
        || ["resume", "cv", "candidate", "recruit"]
            .iter()
            .any(|hint| lower_prompt.contains(hint))
}

fn prompt_requests_dimension_statistics(
    prompt: &str,
    dimension_hints: &[&str],
    ascii_dimension_hints: &[&str],
) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    let has_dimension_signal = prompt_contains_any(prompt, dimension_hints)
        || ascii_prompt_contains_any(&lower_prompt, ascii_dimension_hints);
    let has_stat_signal = prompt_contains_any(
        prompt,
        &[
            "多少", "几个", "哪些", "列出", "统计", "汇总", "全部", "所有", "清单", "覆盖", "出现",
            "提到", "频次", "频率", "排序", "排行", "表", "表格", "按",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "count",
            "list",
            "all",
            "which",
            "inventory",
            "coverage",
            "frequency",
            "frequencies",
            "sort",
            "rank",
            "table",
        ],
    ) || lower_prompt.contains("how many");

    has_dimension_signal && has_stat_signal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_statistics_keep_chinese_and_ascii_signals() {
        assert!(prompt_requests_skill_statistics("统计简历里的技能"));
        assert!(prompt_requests_project_statistics(
            "list projects from resumes"
        ));
        assert!(prompt_requests_location_statistics("按城市出表"));
        assert!(prompt_requests_certificate_statistics(
            "certificate inventory"
        ));
        assert!(!prompt_requests_skill_statistics("这个人会 Rust 吗"));
    }

    #[test]
    fn table_statistics_require_table_and_document_context() {
        assert!(prompt_requests_table_statistics("列出文档里的表格结构"));
        assert!(prompt_requests_table_statistics(
            "scan documents and list tabular structures"
        ));
        assert!(!prompt_requests_table_statistics("做一个经营报表"));
        assert!(!prompt_requests_table_statistics("文档有哪些章节"));
    }

    #[test]
    fn resume_signal_and_time_statistics_keep_existing_boundaries() {
        assert!(prompt_has_resume_signal("候选人简历分析"));
        assert!(prompt_has_resume_signal("candidate resume timeline"));
        assert!(prompt_requests_resume_time_statistics("简历按最近年份排序"));
        assert!(!prompt_requests_resume_time_statistics("按最近年份统计"));
    }

    #[test]
    fn keyword_year_section_paragraph_age_gender_statistics_are_preserved() {
        assert!(prompt_requests_keyword_statistics("列出关键词清单"));
        assert!(prompt_requests_year_statistics("按年份统计"));
        assert!(prompt_requests_section_statistics("列出文档结构标题"));
        assert!(prompt_requests_paragraph_statistics("统计正文段落"));
        assert!(prompt_requests_age_statistics("按年龄排序"));
        assert!(prompt_requests_gender_statistics("统计候选人性别"));
    }
}
