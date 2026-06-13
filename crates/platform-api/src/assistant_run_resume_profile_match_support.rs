use serde_json::Value;

use crate::{
    ascii_prompt_contains_any, assistant_run_resume_profile_table, escape_markdown_table_cell,
    is_ascii_connector_token_char, is_document_entity_noise, is_valid_company_name,
    known_location_names, lexical_query_tokens, normalize_document_entity_value,
    prompt_contains_any, prompt_requests_certificate_statistics, prompt_requests_degree_statistics,
    prompt_requests_location_statistics, prompt_requests_position_statistics,
    prompt_requests_project_statistics, prompt_requests_school_statistics,
    prompt_requests_skill_statistics, resume_profile_array_string, resume_profile_array_values,
    resume_profile_candidate_name, value_i64_string, value_string, value_u64_string,
};

#[derive(Clone, Debug)]
pub(crate) struct ResumeProfileMatchCriterion {
    pub(crate) field_key: &'static str,
    pub(crate) field_label: &'static str,
    pub(crate) term: String,
}

pub(crate) fn resume_profile_match_criteria(
    prompt: &str,
    rows: &[Value],
) -> Vec<ResumeProfileMatchCriterion> {
    let preferred_fields = resume_profile_preferred_match_fields(prompt);
    let mut criteria = Vec::new();
    for (field_key, field_label) in preferred_fields {
        for term in resume_profile_unique_field_terms(rows, field_key) {
            if let Some(matched_term) = resume_profile_prompt_match_term(prompt, &term) {
                criteria.push(ResumeProfileMatchCriterion {
                    field_key,
                    field_label,
                    term: matched_term,
                });
                break;
            }
        }
    }
    criteria
}

fn resume_profile_preferred_match_fields(prompt: &str) -> Vec<(&'static str, &'static str)> {
    let mut fields = Vec::new();
    if prompt_requests_skill_statistics(prompt)
        || prompt_contains_any(prompt, &["会", "懂", "熟悉", "掌握", "技术栈"])
    {
        fields.push(("skill_names", "技能"));
    }
    if prompt_requests_project_statistics(prompt)
        || prompt_contains_any(prompt, &["做过", "项目", "产品", "案例", "经历", "参与"])
    {
        fields.push(("project_names", "项目"));
    }
    if prompt_contains_any(prompt, &["公司", "任职", "雇主", "工作过", "待过"])
        || ascii_prompt_contains_any(&prompt.to_ascii_lowercase(), &["company", "employer"])
    {
        fields.push(("company_names", "公司"));
    }
    if prompt_requests_position_statistics(prompt)
        || prompt_contains_any(prompt, &["岗位", "职位", "职务", "角色"])
    {
        fields.push(("position_names", "岗位"));
    }
    if prompt_requests_location_statistics(prompt)
        || prompt_contains_any(prompt, &["城市", "地点", "地区", "所在地"])
    {
        fields.push(("location_names", "地点"));
    }
    if prompt_requests_school_statistics(prompt)
        || prompt_contains_any(prompt, &["学校", "院校", "毕业院校", "大学", "学院"])
    {
        fields.push(("school_names", "学校"));
    }
    if prompt_requests_degree_statistics(prompt)
        || prompt_contains_any(prompt, &["学历", "学位", "本科", "硕士", "博士", "大专"])
    {
        fields.push(("degree_names", "学历"));
    }
    if prompt_requests_certificate_statistics(prompt)
        || prompt_contains_any(prompt, &["证书", "认证", "资质", "资格"])
    {
        fields.push(("certificate_names", "证书"));
    }
    if fields.is_empty() {
        fields.extend([
            ("skill_names", "技能"),
            ("project_names", "项目"),
            ("company_names", "公司"),
            ("position_names", "岗位"),
            ("location_names", "地点"),
            ("school_names", "学校"),
            ("degree_names", "学历"),
            ("certificate_names", "证书"),
        ]);
    }
    dedupe_resume_profile_match_fields(fields)
}

fn dedupe_resume_profile_match_fields(
    fields: Vec<(&'static str, &'static str)>,
) -> Vec<(&'static str, &'static str)> {
    let mut deduped = Vec::new();
    for field in fields {
        if !deduped
            .iter()
            .any(|(field_key, _): &(&str, &str)| *field_key == field.0)
        {
            deduped.push(field);
        }
    }
    deduped
}

fn resume_profile_unique_field_terms(rows: &[Value], field_key: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for row in rows {
        for value in resume_profile_array_values(row, field_key) {
            if resume_profile_term_can_filter(&value) && !terms.iter().any(|term| term == &value) {
                terms.push(value);
            }
        }
    }
    terms.sort_by(|left, right| {
        right
            .chars()
            .count()
            .cmp(&left.chars().count())
            .then_with(|| left.cmp(right))
    });
    terms
}

pub(crate) fn resume_profile_row_matches_criterion(
    row: &Value,
    criterion: &ResumeProfileMatchCriterion,
) -> bool {
    resume_profile_array_values(row, criterion.field_key)
        .into_iter()
        .any(|value| resume_profile_terms_match(&value, &criterion.term))
}

pub(crate) fn resume_profile_row_match_score(
    row: &Value,
    criteria: &[ResumeProfileMatchCriterion],
) -> usize {
    criteria
        .iter()
        .map(|criterion| {
            resume_profile_array_values(row, criterion.field_key)
                .into_iter()
                .filter(|value| resume_profile_terms_match(value, &criterion.term))
                .count()
        })
        .sum()
}

pub(crate) fn resume_profile_row_match_summary(
    row: &Value,
    criteria: &[ResumeProfileMatchCriterion],
) -> String {
    let mut matches = Vec::new();
    for criterion in criteria {
        for value in resume_profile_array_values(row, criterion.field_key) {
            if resume_profile_terms_match(&value, &criterion.term) {
                let item = format!("{}:{}", criterion.field_label, value);
                if !matches.iter().any(|existing| existing == &item) {
                    matches.push(item);
                }
            }
        }
    }
    if matches.is_empty() {
        "-".to_string()
    } else {
        matches.into_iter().take(5).collect::<Vec<_>>().join("；")
    }
}

pub(crate) fn assistant_run_resume_profile_match_answer(
    mut rows: Vec<Value>,
    prompt: &str,
) -> Option<String> {
    let criteria = resume_profile_match_criteria(prompt, &rows);
    if criteria.is_empty() {
        return None;
    }

    rows.retain(|row| {
        criteria
            .iter()
            .all(|criterion| resume_profile_row_matches_criterion(row, criterion))
    });
    rows.sort_by(|left, right| {
        resume_profile_row_match_score(right, &criteria)
            .cmp(&resume_profile_row_match_score(left, &criteria))
            .then_with(|| {
                right
                    .get("latest_year")
                    .and_then(Value::as_i64)
                    .cmp(&left.get("latest_year").and_then(Value::as_i64))
            })
            .then_with(|| {
                right
                    .get("skill_count")
                    .and_then(Value::as_u64)
                    .cmp(&left.get("skill_count").and_then(Value::as_u64))
            })
            .then_with(|| {
                resume_profile_candidate_name(left).cmp(&resume_profile_candidate_name(right))
            })
    });

    let criteria_label = criteria
        .iter()
        .map(|criterion| format!("{}={}", criterion.field_label, criterion.term))
        .collect::<Vec<_>>()
        .join("，");
    if rows.is_empty() {
        return Some(format!(
            "未在可见简历结构化扫描中找到匹配“{}”的候选人。",
            escape_markdown_table_cell(&criteria_label)
        ));
    }

    Some(assistant_run_resume_profile_table(
        &rows,
        &format!("匹配“{criteria_label}”的候选人简历表"),
        &[
            "候选人",
            "匹配项",
            "技能",
            "项目",
            "公司",
            "岗位",
            "地点",
            "学历",
            "证书",
            "年龄",
            "最近年份",
            "文档",
        ],
        |row| {
            vec![
                resume_profile_candidate_name(row),
                resume_profile_row_match_summary(row, &criteria),
                resume_profile_array_string(row, "skill_names", 4),
                resume_profile_array_string(row, "project_names", 3),
                resume_profile_array_string(row, "company_names", 3),
                resume_profile_array_string(row, "position_names", 2),
                resume_profile_array_string(row, "location_names", 2),
                resume_profile_array_string(row, "degree_names", 2),
                resume_profile_array_string(row, "certificate_names", 2),
                value_u64_string(row, "age"),
                value_i64_string(row, "latest_year"),
                value_string(row, "document_title"),
            ]
        },
    ))
}

fn resume_profile_prompt_match_term(prompt: &str, term: &str) -> Option<String> {
    let prompt_text = prompt.to_ascii_lowercase();
    let normalized_term = normalize_document_entity_value(term);
    let term_text = normalized_term.to_ascii_lowercase();
    if term_text.is_empty() || !resume_profile_term_can_filter(&term_text) {
        return None;
    }
    if is_valid_company_name(&normalized_term) {
        return prompt_text.contains(&term_text).then_some(normalized_term);
    }
    if resume_profile_term_is_ascii(&term_text) {
        return lexical_query_tokens(&prompt_text)
            .into_iter()
            .any(|token| token == term_text)
            .then_some(normalized_term);
    }
    if prompt_text.contains(&term_text) {
        return Some(normalized_term);
    }
    let mut tokens = lexical_query_tokens(&prompt_text)
        .into_iter()
        .filter(|token| resume_profile_query_token_can_filter(token))
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| {
        right
            .chars()
            .count()
            .cmp(&left.chars().count())
            .then_with(|| left.cmp(right))
    });
    tokens.into_iter().find(|token| term_text.contains(token))
}

fn resume_profile_terms_match(value: &str, term: &str) -> bool {
    let value_text = normalize_document_entity_value(value).to_ascii_lowercase();
    let term_text = normalize_document_entity_value(term).to_ascii_lowercase();
    if value_text.is_empty() || term_text.is_empty() {
        return false;
    }
    if is_valid_company_name(&normalize_document_entity_value(term)) {
        return value_text == term_text;
    }
    if resume_profile_term_is_ascii(&term_text) {
        return lexical_query_tokens(&value_text)
            .into_iter()
            .any(|token| token == term_text);
    }
    value_text == term_text
        || value_text.contains(&term_text)
        || term_text.contains(&value_text)
        || lexical_query_tokens(&term_text)
            .into_iter()
            .filter(|token| resume_profile_query_token_can_filter(token))
            .any(|token| value_text.contains(&token))
}

fn resume_profile_term_is_ascii(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || is_ascii_connector_token_char(ch))
}

fn resume_profile_term_can_filter(value: &str) -> bool {
    let normalized = normalize_document_entity_value(value);
    let lower = normalized.to_ascii_lowercase();
    let char_count = normalized.chars().count();
    (2..=80).contains(&char_count)
        && !is_document_entity_noise(&normalized)
        && ![
            "候选人",
            "简历",
            "人才",
            "人员",
            "项目",
            "项目经验",
            "技能",
            "公司",
            "岗位",
            "职位",
            "经验",
            "熟悉",
            "掌握",
            "负责",
            "参与",
            "做过",
            "会",
            "懂",
            "with",
            "has",
            "have",
        ]
        .contains(&lower.as_str())
}

fn resume_profile_query_token_can_filter(token: &str) -> bool {
    let char_count = token.chars().count();
    char_count >= 2
        && resume_profile_term_can_filter(token)
        && !known_location_names()
            .iter()
            .any(|location| token == *location)
        && ![
            "哪些",
            "哪个",
            "哪位",
            "谁会",
            "谁有",
            "有无",
            "有没有",
            "做过",
            "熟悉",
            "掌握",
            "项目",
            "经验",
            "技能",
            "公司",
            "平台",
            "系统",
            "产品",
            "经理",
            "候选",
            "候选人",
            "简历",
            "人才",
            "人员",
            "有限",
            "有限公",
            "有限公司",
            "网络",
            "科技",
            "顾问",
            "地产",
        ]
        .contains(&token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn criteria_prefers_prompted_fields_and_longer_terms() {
        let rows = vec![
            json!({
                "candidate_name": "张三",
                "skill_names": ["Rust", "React"],
                "project_names": ["智能知识库平台", "知识库"]
            }),
            json!({
                "candidate_name": "李四",
                "skill_names": ["Java"],
                "project_names": ["ERP"]
            }),
        ];

        let criteria = resume_profile_match_criteria("谁做过智能知识库平台项目", &rows);

        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].field_key, "project_names");
        assert_eq!(criteria[0].field_label, "项目");
        assert_eq!(criteria[0].term, "智能知识库平台");
    }

    #[test]
    fn row_match_summary_dedupes_and_scores_matches() {
        let criteria = vec![
            ResumeProfileMatchCriterion {
                field_key: "skill_names",
                field_label: "技能",
                term: "Rust".to_string(),
            },
            ResumeProfileMatchCriterion {
                field_key: "project_names",
                field_label: "项目",
                term: "知识库".to_string(),
            },
        ];
        let row = json!({
            "skill_names": ["Rust", "Rust"],
            "project_names": ["智能知识库平台"]
        });

        assert!(resume_profile_row_matches_criterion(&row, &criteria[0]));
        assert_eq!(resume_profile_row_match_score(&row, &criteria), 3);
        assert_eq!(
            resume_profile_row_match_summary(&row, &criteria),
            "技能:Rust；项目:智能知识库平台"
        );
    }

    #[test]
    fn term_matching_handles_ascii_companies_and_noise() {
        assert_eq!(
            resume_profile_prompt_match_term("候选人有没有 Rust 经验", "Rust"),
            Some("Rust".to_string())
        );
        assert_eq!(
            resume_profile_prompt_match_term(
                "是否在广州冠晚网络有限公司工作过",
                "广州冠晚网络有限公司"
            ),
            Some("广州冠晚网络有限公司".to_string())
        );
        assert_eq!(resume_profile_prompt_match_term("谁会项目", "项目"), None);
        assert!(resume_profile_terms_match("Rust / React", "Rust"));
        assert!(!resume_profile_terms_match(
            "广州冠晚网络有限公司深圳分部",
            "广州冠晚网络有限公司"
        ));
    }

    #[test]
    fn match_answer_filters_sorts_and_keeps_original_table_shape() {
        let rows = vec![
            json!({
                "candidate_name": "李四",
                "skill_names": ["Rust"],
                "project_names": ["知识库平台"],
                "company_names": ["甲公司"],
                "position_names": ["后端工程师"],
                "location_names": ["上海"],
                "degree_names": ["本科"],
                "certificate_names": ["PMP"],
                "age": 31,
                "latest_year": 2025,
                "skill_count": 2,
                "document_title": "李四简历.pdf"
            }),
            json!({
                "candidate_name": "张三",
                "skill_names": ["Rust", "React"],
                "project_names": ["知识库平台"],
                "company_names": ["乙公司"],
                "position_names": ["全栈工程师"],
                "location_names": ["深圳"],
                "degree_names": ["硕士"],
                "certificate_names": [],
                "age": 29,
                "latest_year": 2024,
                "skill_count": 4,
                "document_title": "张三简历.pdf"
            }),
            json!({
                "candidate_name": "王五",
                "skill_names": ["Java"],
                "project_names": ["ERP"],
                "latest_year": 2026,
                "skill_count": 6,
                "document_title": "王五简历.pdf"
            }),
        ];

        let answer = assistant_run_resume_profile_match_answer(rows, "谁有 Rust 技能")
            .expect("match answer should be produced");

        assert!(answer.contains("匹配“技能=Rust”的候选人简历表"));
        assert!(answer.contains("| 候选人 | 匹配项 | 技能 | 项目 | 公司 | 岗位 | 地点 | 学历 | 证书 | 年龄 | 最近年份 | 文档 |"));
        assert!(answer.contains("| 张三 | 技能:Rust | Rust；React | 知识库平台 | 乙公司 | 全栈工程师 | 深圳 | 硕士 | - | 29 | 2024 | 张三简历.pdf |"));
        assert!(answer.contains("| 李四 | 技能:Rust | Rust | 知识库平台 | 甲公司 | 后端工程师 | 上海 | 本科 | PMP | 31 | 2025 | 李四简历.pdf |"));
        assert!(!answer.contains("王五"));

        let lisi_index = answer.find("| 李四 |").expect("李四 row should exist");
        let zhang_index = answer.find("| 张三 |").expect("张三 row should exist");
        assert!(
            lisi_index < zhang_index,
            "latest_year should sort before skill_count when match score ties"
        );
    }
}
