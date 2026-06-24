use crate::{
    ascii_prompt_contains_any, known_location_names, prompt_contains_any, prompt_has_resume_signal,
    prompt_requests_certificate_statistics, prompt_requests_degree_statistics,
    prompt_requests_location_statistics, prompt_requests_position_statistics,
    prompt_requests_project_statistics, prompt_requests_school_statistics,
    prompt_requests_skill_statistics,
};

pub(crate) fn prompt_requests_resume_skill_ranking(prompt: &str) -> bool {
    prompt_requests_resume_profile_sort(prompt) && prompt_requests_skill_statistics(prompt)
}

pub(crate) fn prompt_requests_resume_project_ranking(prompt: &str) -> bool {
    prompt_requests_resume_profile_sort(prompt) && prompt_requests_project_statistics(prompt)
}

pub(crate) fn prompt_requests_resume_project_delivery_listing(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    let has_project_signal = prompt_contains_any(
        prompt,
        &[
            "项目交付",
            "交付经历",
            "项目经历",
            "项目经验",
            "项目职责",
            "项目成果",
        ],
    ) || (prompt.contains("项目")
        && prompt_contains_any(prompt, &["交付", "经历", "职责", "成果", "明细", "罗列"]))
        || lower_prompt.contains("project delivery");
    if !has_project_signal {
        return false;
    }
    let has_people_or_resume_signal = prompt_has_resume_signal(prompt)
        || prompt_contains_any(
            prompt,
            &[
                "个人",
                "这些人",
                "这些候选",
                "这几个人",
                "这批人",
                "候选",
                "人才",
                "求职者",
            ],
        )
        || lower_prompt.contains("people")
        || lower_prompt.contains("candidate");
    let has_listing_signal =
        prompt_contains_any(
            prompt,
            &[
                "罗列",
                "列出",
                "列一下",
                "整理",
                "汇总",
                "明细",
                "清单",
                "出表",
                "表格",
                "全部",
                "所有",
            ],
        ) || ascii_prompt_contains_any(&lower_prompt, &["list", "table", "summary", "all"]);
    has_people_or_resume_signal && has_listing_signal
}

pub(crate) fn prompt_requests_resume_position_ranking(prompt: &str) -> bool {
    if !prompt_requests_resume_profile_sort(prompt) {
        return false;
    }
    prompt_requests_position_statistics(prompt)
        || prompt_contains_any(
            prompt,
            &["岗位数", "岗位数量", "职位数", "职位数量", "角色数量"],
        )
}

pub(crate) fn prompt_requests_resume_location_ranking(prompt: &str) -> bool {
    if !prompt_requests_resume_profile_sort(prompt) {
        return false;
    }
    prompt_requests_location_statistics(prompt)
        || prompt_contains_any(
            prompt,
            &[
                "城市数",
                "城市数量",
                "地点数",
                "地点数量",
                "地区数",
                "地区数量",
            ],
        )
}

pub(crate) fn prompt_requests_resume_company_ranking(prompt: &str) -> bool {
    if !prompt_requests_resume_profile_sort(prompt) {
        return false;
    }
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_contains_any(prompt, &["公司数", "公司数量", "任职公司", "工作公司"])
        || ascii_prompt_contains_any(&lower_prompt, &["company_count", "companies", "employers"])
}

pub(crate) fn prompt_requests_resume_education_ranking(prompt: &str) -> bool {
    if !prompt_requests_resume_profile_sort(prompt) {
        return false;
    }
    prompt_requests_school_statistics(prompt)
        || prompt_requests_degree_statistics(prompt)
        || prompt_contains_any(
            prompt,
            &[
                "教育经历",
                "教育背景",
                "学历",
                "学位",
                "学校",
                "院校",
                "毕业院校",
            ],
        )
}

pub(crate) fn prompt_requests_resume_certificate_ranking(prompt: &str) -> bool {
    if !prompt_requests_resume_profile_sort(prompt) {
        return false;
    }
    prompt_requests_certificate_statistics(prompt)
        || prompt_contains_any(prompt, &["证书数", "证书数量", "资质数量", "认证数量"])
}

pub(crate) fn prompt_requests_resume_experience_statistics(prompt: &str) -> bool {
    if !prompt_requests_resume_profile_sort(prompt) {
        return false;
    }
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_contains_any(prompt, &["工作年限", "经验年限", "年限", "履历年限"])
        || ascii_prompt_contains_any(&lower_prompt, &["experience", "tenure", "duration"])
}

pub(crate) fn prompt_requests_resume_profile_match(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    if prompt_requests_resume_profile_sort(prompt)
        || prompt_contains_any(
            prompt,
            &["出现频次", "频次", "频率", "覆盖文档", "覆盖", "去重"],
        )
        || ascii_prompt_contains_any(
            &lower_prompt,
            &["frequency", "frequencies", "coverage", "dedupe"],
        )
    {
        return false;
    }

    let has_candidate_target = prompt_contains_any(
        prompt,
        &[
            "谁",
            "哪位",
            "哪些人",
            "哪些候选人",
            "候选人",
            "人才",
            "人选",
            "人员",
            "求职者",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &["who", "candidate", "candidates", "person", "people"],
    );
    let has_resume_search = prompt_has_resume_signal(prompt)
        && (prompt_contains_any(
            prompt,
            &["找", "筛", "筛选", "匹配", "推荐", "符合", "满足"],
        ) || ascii_prompt_contains_any(
            &lower_prompt,
            &["find", "match", "filter", "recommend"],
        ));
    let has_match_signal = prompt_contains_any(
        prompt,
        &[
            "会",
            "懂",
            "熟悉",
            "掌握",
            "具备",
            "有",
            "做过",
            "参与",
            "负责",
            "任职",
            "工作过",
            "待过",
            "经验",
            "技能",
            "技术栈",
            "项目",
            "公司",
            "岗位",
            "职位",
            "地点",
            "城市",
            "地区",
            "所在地",
            "地址",
            "学校",
            "院校",
            "学历",
            "学位",
            "证书",
            "认证",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "with",
            "has",
            "have",
            "knows",
            "skill",
            "skills",
            "project",
            "company",
            "employer",
            "role",
            "location",
            "city",
            "school",
            "university",
            "degree",
            "certificate",
            "certification",
        ],
    ) || known_location_names()
        .iter()
        .any(|location| prompt.contains(location));

    (has_candidate_target || has_resume_search) && has_match_signal
}

pub(crate) fn prompt_requests_resume_candidate_recommendation(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    if prompt_prefers_entity_frequency(prompt) {
        return false;
    }
    if prompt_contains_any(
        prompt,
        &[
            "技能数",
            "技能数量",
            "项目数",
            "项目数量",
            "公司数",
            "公司数量",
            "城市数",
            "城市数量",
            "岗位数",
            "岗位数量",
            "证书数",
            "证书数量",
            "工作年限",
            "经验年限",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "skill_count",
            "project_count",
            "company_count",
            "position_count",
            "certificate_count",
        ],
    ) {
        return false;
    }

    let has_people_or_resume_signal = prompt_has_resume_signal(prompt)
        || prompt_contains_any(
            prompt,
            &[
                "个人",
                "这些人",
                "这几个人",
                "这批人",
                "这组人",
                "这些候选",
                "人选",
                "人员",
                "求职者",
            ],
        )
        || ascii_prompt_contains_any(
            &lower_prompt,
            &["candidate", "candidates", "people", "person", "applicant"],
        );
    if !has_people_or_resume_signal {
        return false;
    }

    let has_selection_signal = prompt_contains_any(
        prompt,
        &[
            "最合适",
            "最适合",
            "最匹配",
            "最推荐",
            "哪一个",
            "哪一位",
            "哪个",
            "哪位",
            "谁最",
            "排名",
            "排行",
            "排序",
            "优先",
            "推荐",
            "筛选",
            "比较",
            "对比",
            "候选",
            "人选",
            "负责",
            "招聘",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "best",
            "fit",
            "suitable",
            "recommend",
            "rank",
            "ranking",
            "compare",
            "candidate",
            "shortlist",
        ],
    );
    let has_role_or_capability_signal = prompt_contains_any(
        prompt,
        &[
            "负责",
            "岗位",
            "职位",
            "角色",
            "开发",
            "工程师",
            "产品",
            "项目",
            "平台",
            "系统",
            "算法",
            "数据",
            "智能",
            "技术",
            "技能",
            "经验",
            "业务",
            "适合",
            "匹配",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "java",
            "python",
            "rust",
            "go",
            "ai",
            "llm",
            "agent",
            "backend",
            "frontend",
            "fullstack",
            "platform",
            "system",
            "project",
            "product",
            "developer",
            "engineer",
            "skill",
            "experience",
        ],
    );

    has_selection_signal && has_role_or_capability_signal
}

fn prompt_requests_resume_profile_sort(prompt: &str) -> bool {
    if !prompt_has_resume_signal(prompt) || prompt_prefers_entity_frequency(prompt) {
        return false;
    }
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_contains_any(
        prompt,
        &[
            "排序",
            "排行",
            "排名",
            "出表",
            "表格",
            "按",
            "候选人",
            "人才",
        ],
    ) || ascii_prompt_contains_any(&lower_prompt, &["sort", "rank", "table", "by"])
}

pub(crate) fn prompt_requests_ascending_sort(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_contains_any(
        prompt,
        &[
            "升序",
            "从小到大",
            "小到大",
            "低到高",
            "少到多",
            "从少到多",
            "从早到晚",
            "早到晚",
            "最年轻",
            "年轻",
            "年龄小",
        ],
    ) || ascii_prompt_contains_any(&lower_prompt, &["asc", "ascending", "youngest"])
        || lower_prompt.contains("low to high")
        || lower_prompt.contains("small to large")
}

fn prompt_prefers_entity_frequency(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_contains_any(
        prompt,
        &[
            "出现频次",
            "频次",
            "频率",
            "覆盖",
            "覆盖文档",
            "哪些",
            "有哪些",
            "清单",
            "去重",
            "提到",
        ],
    ) || ascii_prompt_contains_any(
        &lower_prompt,
        &[
            "frequency",
            "frequencies",
            "coverage",
            "which",
            "list",
            "dedupe",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_sort_accepts_ranking_and_rejects_frequency_lists() {
        assert!(prompt_requests_resume_skill_ranking("简历按技能数排序出表"));
        assert!(prompt_requests_resume_company_ranking(
            "候选人按公司数量排行"
        ));
        assert!(!prompt_requests_resume_skill_ranking(
            "简历里有哪些技能，按出现频次列清单"
        ));
    }

    #[test]
    fn project_delivery_listing_requires_people_and_listing_signals() {
        assert!(prompt_requests_resume_project_delivery_listing(
            "这批候选人的项目交付经历整理成表格"
        ));
        assert!(prompt_requests_resume_project_delivery_listing(
            "candidate project delivery summary"
        ));
        assert!(!prompt_requests_resume_project_delivery_listing(
            "项目交付是什么意思"
        ));
        assert!(!prompt_requests_resume_project_delivery_listing(
            "候选人有没有项目经验"
        ));
    }

    #[test]
    fn profile_match_distinguishes_search_from_sort_or_frequency() {
        assert!(prompt_requests_resume_profile_match(
            "谁会 Rust 并做过知识库项目"
        ));
        assert!(prompt_requests_resume_profile_match(
            "简历里筛选城市为深圳的人"
        ));
        assert!(!prompt_requests_resume_profile_match("简历按技能数排序"));
        assert!(!prompt_requests_resume_profile_match(
            "简历技能出现频次和覆盖文档统计"
        ));
    }

    #[test]
    fn candidate_recommendation_detects_multi_resume_selection() {
        assert!(prompt_requests_resume_candidate_recommendation(
            "如果我要招聘一名JAVA开发，负责AI平台开发，这14个人，哪一个最合适；提供一下排名及原因"
        ));
        assert!(prompt_requests_resume_candidate_recommendation(
            "这几份简历里推荐一个最适合做算法工程师的人选"
        ));
        assert!(!prompt_requests_resume_candidate_recommendation(
            "简历技能出现频次和覆盖文档统计"
        ));
        assert!(!prompt_requests_resume_candidate_recommendation(
            "简历按技能数排序"
        ));
    }

    #[test]
    fn ranking_and_sort_direction_helpers_keep_existing_signals() {
        assert!(prompt_requests_resume_position_ranking("简历按岗位数出表"));
        assert!(prompt_requests_resume_location_ranking(
            "候选人按城市数量排名"
        ));
        assert!(prompt_requests_resume_education_ranking("简历按学历排序"));
        assert!(prompt_requests_resume_certificate_ranking(
            "简历按证书数量排序"
        ));
        assert!(prompt_requests_resume_experience_statistics(
            "简历按工作年限从小到大排序"
        ));
        assert!(prompt_requests_ascending_sort("按年龄从小到大"));
        assert!(prompt_requests_ascending_sort("sort ascending by age"));
    }
}
