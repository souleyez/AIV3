use crate::prompt_match_support::prompt_contains_any;
use crate::retrieval_evidence_ranking_support::vector_norm;
use std::collections::{BTreeMap, BTreeSet};

const ASSISTANT_RUN_LEXICAL_CJK_NGRAM_MAX: usize = 6;

pub(crate) fn prompt_requests_procedure_or_action(prompt: &str) -> bool {
    prompt_contains_any(
        prompt,
        &[
            "怎么办",
            "怎么处理",
            "如何处理",
            "第一时间",
            "流程",
            "步骤",
            "应急",
            "处置",
            "处理规范",
            "操作规范",
            "风险",
            "注意事项",
        ],
    )
}

pub(crate) fn prompt_contains_elder_fall_signal(prompt: &str) -> bool {
    prompt_contains_any(
        prompt,
        &[
            "摔倒",
            "跌倒",
            "摔伤",
            "跌伤",
            "坠床",
            "骨折",
            "意外伤害",
            "突发事件",
            "人身意外",
        ],
    )
}

pub(crate) fn prompt_contains_elder_death_signal(prompt: &str) -> bool {
    prompt_contains_any(
        prompt,
        &[
            "离世",
            "去世",
            "死亡",
            "身故",
            "过世",
            "病故",
            "善后",
            "殡葬",
            "遗体",
            "遗物",
            "家属对接",
            "生命体征",
        ],
    )
}

pub(crate) fn assistant_run_prompt_requests_expanded_supply(prompt: &str) -> bool {
    prompt_requests_procedure_or_action(prompt)
        || prompt_contains_elder_fall_signal(prompt)
        || prompt_contains_elder_death_signal(prompt)
}

pub(crate) fn assistant_run_expanded_supply_prompt(prompt: &str) -> String {
    let mut parts = vec![prompt.trim().to_string()];
    if prompt_requests_procedure_or_action(prompt) {
        parts.push(
            "流程 步骤 处理 处置 应急 措施 要求 报告 记录 通知 医护 家属 现场评估".to_string(),
        );
    }
    if prompt_contains_elder_fall_signal(prompt) {
        parts.push(
            "摔倒 跌倒 防跌倒 坠床 防坠床 摔伤 跌伤 骨折 老年人人身意外伤害 突发事件 应急处置 事故处理 120 通知家属 通知主管领导 医护人员 转院 记录 报告 不要急于扶起"
                .to_string(),
        );
    }
    if prompt_contains_elder_death_signal(prompt) {
        parts.push(
            "离世 去世 死亡 身故 病故 善后 殡葬 遗体 遗物 生命体征 医护确认 现场保护 通知负责人 通知家属 家属沟通 家属对接 死亡证明 殡仪接运 遗物清点 遗物交接 记录归档 事件报告"
                .to_string(),
        );
    }
    parts
        .into_iter()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) struct AssistantRunRankQuery {
    pub(crate) text: String,
    pub(crate) weights: BTreeMap<String, f64>,
    pub(crate) norm: f64,
}

pub(crate) fn assistant_run_rank_query_variants(prompt: &str) -> Vec<AssistantRunRankQuery> {
    let mut texts = Vec::new();
    let mut seen = BTreeSet::new();
    for text in
        std::iter::once(prompt.trim().to_string()).chain(assistant_run_reduced_query_texts(prompt))
    {
        let text = normalize_assistant_reduced_query_text(&text);
        if text.is_empty() || !seen.insert(text.clone()) {
            continue;
        }
        let weights = lexical_query_term_weights(&text);
        let norm = vector_norm(&weights);
        if norm > 0.0 {
            texts.push(AssistantRunRankQuery {
                text,
                weights,
                norm,
            });
        }
    }
    texts
}

pub(crate) fn assistant_run_reduced_query_texts(prompt: &str) -> Vec<String> {
    let mut variants = Vec::new();
    let stripped = assistant_run_strip_question_noise(prompt);
    if !stripped.is_empty() && stripped != prompt.trim() {
        variants.push(stripped.clone());
    }

    let key_terms = assistant_run_key_terms_query(prompt, 10);
    if !key_terms.is_empty() {
        variants.push(key_terms);
    }

    if prompt_requests_procedure_or_action(prompt) && !stripped.is_empty() {
        variants.push(format!("{stripped} 处理 处置 应急 流程"));
    }
    if prompt_contains_elder_fall_signal(prompt) {
        variants.push("摔倒 跌倒 意外伤害 突发事件 事故处理 应急处置 120 通知家属".to_string());
    }
    if prompt_contains_elder_death_signal(prompt) {
        variants.push(
            "离世 去世 死亡 身故 善后 遗体 遗物 医护确认 生命体征 通知家属 家属沟通 殡仪接运 遗物交接 记录归档"
                .to_string(),
        );
    }

    let mut seen = BTreeSet::new();
    variants
        .into_iter()
        .map(|text| normalize_assistant_reduced_query_text(&text))
        .filter(|text| !text.is_empty() && seen.insert(text.clone()))
        .collect()
}

pub(crate) fn assistant_run_strip_question_noise(prompt: &str) -> String {
    let mut text = prompt.trim().to_string();
    for noise in [
        "请问",
        "帮我",
        "帮忙",
        "回答",
        "总结",
        "一下",
        "这个",
        "那个",
        "这份",
        "文档里",
        "文档里面",
        "资料里",
        "资料里面",
        "问题",
        "是什么",
        "是谁",
        "怎么办",
        "怎么处理",
        "如何处理",
        "如何",
        "哪些",
        "需要",
        "可以",
        "能不能",
        "应该",
        "到底",
        "吗",
        "呢",
    ] {
        text = text.replace(noise, " ");
    }
    normalize_assistant_reduced_query_text(&text)
}

pub(crate) fn assistant_run_key_terms_query(prompt: &str, limit: usize) -> String {
    let mut terms = lexical_query_term_weights(prompt)
        .into_iter()
        .filter(|(term, _)| assistant_run_reduced_query_term_is_signal(term))
        .collect::<Vec<_>>();
    terms.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.0.chars().count().cmp(&left.0.chars().count()))
            .then_with(|| left.0.cmp(&right.0))
    });
    terms
        .into_iter()
        .map(|(term, _)| term)
        .take(limit)
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn assistant_run_reduced_query_term_is_signal(term: &str) -> bool {
    let char_count = term.chars().count();
    if char_count < 2 || assistant_run_reduced_query_term_is_noise(term) {
        return false;
    }
    if term.chars().all(is_cjk_query_token_char) {
        return char_count >= 2;
    }
    term.chars().any(|value| value.is_ascii_alphanumeric())
}

fn assistant_run_reduced_query_term_is_noise(term: &str) -> bool {
    matches!(
        term,
        "请问"
            | "帮我"
            | "帮忙"
            | "回答"
            | "总结"
            | "一下"
            | "这个"
            | "那个"
            | "这份"
            | "文档"
            | "资料"
            | "里面"
            | "问题"
            | "是什么"
            | "是谁"
            | "怎么办"
            | "怎么处理"
            | "如何"
            | "哪些"
            | "需要"
            | "可以"
            | "能不能"
            | "应该"
            | "到底"
    )
}

pub(crate) fn normalize_assistant_reduced_query_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn lexical_domain_hint_score(content: &str, query: &str) -> f64 {
    let medication_dispense_query = query.contains("发药")
        || query.contains("服药")
        || query.contains("用药")
        || query.contains("药品")
        || (query.contains("药") && (query.contains("核对") || query.contains("发放")));
    let elder_fall_query = prompt_contains_elder_fall_signal(query);
    let elder_death_query = prompt_contains_elder_death_signal(query);
    let nursing_handover_query = prompt_contains_nursing_handover_signal(query);
    if !medication_dispense_query
        && !elder_fall_query
        && !elder_death_query
        && !nursing_handover_query
    {
        return 0.0;
    }
    if content.contains("................................................................") {
        return 0.0;
    }

    let mut score = 0.0;
    if medication_dispense_query
        && (content.contains("老人自带药品管理规范")
            || content.contains("老年人自带药品")
            || content.contains("药品发放人员")
            || content.contains("2.4 发药")
            || (content.contains("备药") && content.contains("发药")))
    {
        score += 0.5;
    } else if medication_dispense_query
        && (content.contains("药品委托发放") || content.contains("药品统一管理风险告知"))
    {
        score += 0.25;
    } else if medication_dispense_query
        && (content.contains("协助老年人用药") || content.contains("用药安全"))
    {
        score += 0.12;
    }

    if medication_dispense_query
        && (query.contains("核对") || query.contains("查对"))
        && (content.contains("核对信息")
            || content.contains("核对确认")
            || content.contains("药品名称")
            || content.contains("药品使用剂量"))
    {
        score += 0.2;
    }

    if elder_fall_query {
        if content.contains("老年人人身意外伤害")
            || content.contains("突发事件应急预防与处置")
            || content.contains("典型突发事件应急处置")
            || content.contains("事故处理与报告")
            || content.contains("事故处理")
        {
            score += 0.65;
        } else if content.contains("防跌倒")
            || content.contains("跌倒")
            || content.contains("坠床")
            || content.contains("骨折")
            || content.contains("摔伤")
        {
            score += 0.25;
        }
        if prompt_requests_procedure_or_action(query)
            && (content.contains("120")
                || content.contains("通知")
                || content.contains("报告")
                || content.contains("记录")
                || content.contains("转院")
                || content.contains("医护人员"))
        {
            score += 0.3;
        }
    }

    if elder_death_query {
        if content.contains("离世")
            || content.contains("去世")
            || content.contains("死亡")
            || content.contains("身故")
            || content.contains("病故")
            || content.contains("善后")
            || content.contains("遗体")
            || content.contains("殡仪")
            || content.contains("殡葬")
        {
            score += 0.65;
        }
        if prompt_requests_procedure_or_action(query)
            && (content.contains("生命体征")
                || content.contains("医护确认")
                || content.contains("通知家属")
                || content.contains("联系家属")
                || content.contains("保护现场")
                || content.contains("遗物")
                || content.contains("交接")
                || content.contains("记录")
                || content.contains("归档"))
        {
            score += 0.35;
        }
    }

    if nursing_handover_query {
        if content.contains("四、交接内容")
            || content.contains("护理记录单")
            || content.contains("晨会交接记录本")
            || content.contains("交班护理员")
            || content.contains("接班护理员")
        {
            score += 0.75;
        } else if content.contains("交接班制度")
            || content.contains("交接及处置记录")
            || content.contains("床旁交接班")
        {
            score += 0.2;
        }

        if content.contains("药物服用情况")
            || content.contains("身体异常状况")
            || content.contains("情绪异常状况")
            || content.contains("皮肤受压情况")
            || content.contains("管路通畅")
        {
            score += 0.35;
        }
    }

    score
}

pub(crate) fn lexical_query_term_weights(query: &str) -> BTreeMap<String, f64> {
    lexical_term_weights(lexical_query_tokens(query))
}

/// Produces literal surface-form weights without the domain-hint expansion
/// used by the general retrieval ranker. Semantic graph matching uses this
/// narrower form so inferred synonyms cannot create a graph supply match.
pub(crate) fn lexical_surface_term_weights(content: &str) -> BTreeMap<String, f64> {
    lexical_term_weights(lexical_surface_tokens(content))
}

fn lexical_term_weights(tokens: Vec<String>) -> BTreeMap<String, f64> {
    let mut frequencies: BTreeMap<String, f64> = BTreeMap::new();
    for token in tokens {
        *frequencies.entry(token).or_insert(0.0) += 1.0;
    }

    frequencies
        .into_iter()
        .map(|(term, count)| {
            let boost = lexical_cjk_phrase_boost(&term);
            (term, (1.0 + count.ln()) * boost)
        })
        .collect()
}

pub(crate) fn lexical_query_tokens(content: &str) -> Vec<String> {
    let mut tokens = lexical_surface_tokens(content);
    extend_lexical_domain_hint_tokens(content, &mut tokens);
    tokens
}

fn lexical_surface_tokens(content: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut ascii_token = String::new();
    let mut cjk_chars = Vec::new();

    for value in content.chars() {
        if value.is_ascii_alphanumeric() {
            flush_lexical_cjk_terms(&mut tokens, &mut cjk_chars);
            ascii_token.push(value.to_ascii_lowercase());
            continue;
        }
        flush_lexical_ascii_token(&mut tokens, &mut ascii_token);
        if is_cjk_query_token_char(value) {
            cjk_chars.push(value);
        } else {
            flush_lexical_cjk_terms(&mut tokens, &mut cjk_chars);
        }
    }
    flush_lexical_ascii_token(&mut tokens, &mut ascii_token);
    flush_lexical_cjk_terms(&mut tokens, &mut cjk_chars);
    extend_lexical_ascii_connector_tokens(content, &mut tokens);
    tokens
}

fn extend_lexical_domain_hint_tokens(content: &str, tokens: &mut Vec<String>) {
    let medication_dispense_context = content.contains("发药")
        || content.contains("服药")
        || content.contains("用药")
        || content.contains("代发药")
        || content.contains("药品委托")
        || content.contains("药品发放")
        || (content.contains("委托发放") && content.contains("药"));
    if medication_dispense_context {
        for token in [
            "发药",
            "服药",
            "用药",
            "药品",
            "药物",
            "药品管理",
            "药品委托发放",
            "委托发放",
            "代发",
            "代管",
            "医嘱",
            "剂量",
            "服药禁忌",
            "有效期",
            "标签",
        ] {
            tokens.push(token.to_string());
        }

        if content.contains("核对")
            || content.contains("查对")
            || content.contains("步骤")
            || content.contains("哪些")
        {
            for token in [
                "核对",
                "查对",
                "老年人床号",
                "老年人姓名",
                "药品名称",
                "药品浓度",
                "厂家",
                "数量",
                "剂量",
                "药品使用剂量",
                "药品使用时间",
                "药品使用方法",
                "有效期",
                "禁忌",
                "药品保质期",
                "标签",
            ] {
                tokens.push(token.to_string());
            }
        }
    }

    let elder_fall_context = prompt_contains_elder_fall_signal(content)
        || content.contains("应急处置")
        || content.contains("事故处理")
        || content.contains("通知家属")
        || content.contains("通知主管领导")
        || content.contains("转院")
        || content.contains("医护人员")
        || content.contains("拨打 120")
        || content.contains("拨打120");
    if elder_fall_context {
        for token in [
            "摔倒",
            "跌倒",
            "防跌倒",
            "坠床",
            "防坠床",
            "摔伤",
            "跌伤",
            "骨折",
            "意外伤害",
            "人身意外",
            "老年人人身意外伤害",
            "突发事件",
            "应急处置",
            "事故处理",
            "事故报告",
            "120",
            "通知家属",
            "通知主管领导",
            "医护人员",
            "转院",
            "记录",
            "报告",
            "现场评估",
        ] {
            tokens.push(token.to_string());
        }
    }

    let elder_death_context = prompt_contains_elder_death_signal(content)
        || content.contains("医护确认")
        || content.contains("生命体征")
        || content.contains("保护现场")
        || content.contains("联系家属")
        || content.contains("殡仪")
        || content.contains("遗物交接")
        || content.contains("记录归档");
    if elder_death_context {
        for token in [
            "离世",
            "去世",
            "死亡",
            "身故",
            "病故",
            "过世",
            "善后",
            "殡葬",
            "遗体",
            "遗物",
            "生命体征",
            "医护确认",
            "现场保护",
            "保护现场",
            "通知负责人",
            "通知家属",
            "联系家属",
            "家属沟通",
            "家属对接",
            "死亡证明",
            "殡仪接运",
            "遗物清点",
            "遗物交接",
            "记录归档",
            "事件报告",
        ] {
            tokens.push(token.to_string());
        }
    }

    let nursing_handover_context = prompt_contains_nursing_handover_signal(content)
        || content.contains("晨会交接记录本")
        || content.contains("护理记录单")
        || content.contains("交班护理员")
        || content.contains("接班护理员");
    if nursing_handover_context {
        for token in [
            "护理交接班",
            "交接班",
            "交班",
            "接班",
            "交接内容",
            "晨会交接",
            "护理记录单",
            "药物服用情况",
            "身体异常状况",
            "情绪异常状况",
            "床铺清洁",
            "大小便",
            "皮肤受压",
            "液体速度",
            "吸氧情况",
            "胃管",
            "尿管",
            "引流管",
            "管路通畅",
            "易丢失物品",
            "易损坏物品",
            "送洗衣物",
            "家属探望物品",
            "签字确认",
        ] {
            tokens.push(token.to_string());
        }
    }

    if prompt_requests_procedure_or_action(content) {
        for token in [
            "流程", "步骤", "处理", "处置", "应急", "措施", "要求", "报告", "记录", "通知",
        ] {
            tokens.push(token.to_string());
        }
    }
}

fn prompt_contains_nursing_handover_signal(content: &str) -> bool {
    (content.contains("交接班") || content.contains("护理交接") || content.contains("交班"))
        && (content.contains("护理")
            || content.contains("照护")
            || content.contains("长者")
            || content.contains("老人")
            || content.contains("老年人")
            || content.contains("必须")
            || content.contains("内容")
            || content.contains("哪些"))
}

fn extend_lexical_ascii_connector_tokens(content: &str, tokens: &mut Vec<String>) {
    let mut segment = String::new();
    for value in content.chars() {
        if value.is_ascii_alphanumeric() || is_ascii_connector_token_char(value) {
            segment.push(value.to_ascii_lowercase());
        } else {
            flush_lexical_ascii_connector_token(tokens, &mut segment);
        }
    }
    flush_lexical_ascii_connector_token(tokens, &mut segment);
}

fn flush_lexical_ascii_connector_token(tokens: &mut Vec<String>, segment: &mut String) {
    if segment.is_empty() {
        return;
    }
    if has_internal_ascii_connector(segment) {
        let normalized = segment
            .chars()
            .filter(|value| value.is_ascii_alphanumeric())
            .collect::<String>();
        if let Some(token) = normalize_lexical_query_token(&normalized) {
            tokens.push(token);
        }
    }
    segment.clear();
}

fn has_internal_ascii_connector(segment: &str) -> bool {
    let chars = segment.chars().collect::<Vec<_>>();
    chars.iter().enumerate().any(|(index, value)| {
        is_ascii_connector_token_char(*value)
            && chars[..index]
                .iter()
                .any(|candidate| candidate.is_ascii_alphanumeric())
            && chars[index + 1..]
                .iter()
                .any(|candidate| candidate.is_ascii_alphanumeric())
    })
}

pub(crate) fn is_ascii_connector_token_char(value: char) -> bool {
    matches!(value, '&' | '+' | '/' | '-' | '_' | '.')
}

fn flush_lexical_ascii_token(tokens: &mut Vec<String>, ascii_token: &mut String) {
    if let Some(token) = normalize_lexical_query_token(ascii_token) {
        tokens.push(token);
    }
    ascii_token.clear();
}

fn flush_lexical_cjk_terms(tokens: &mut Vec<String>, cjk_chars: &mut Vec<char>) {
    if cjk_chars.is_empty() {
        return;
    }

    for value in cjk_chars.iter() {
        tokens.push(value.to_string());
    }
    for ngram_size in 2..=ASSISTANT_RUN_LEXICAL_CJK_NGRAM_MAX.min(cjk_chars.len()) {
        for window in cjk_chars.windows(ngram_size) {
            tokens.push(window.iter().collect::<String>());
        }
    }
    cjk_chars.clear();
}

pub(crate) fn is_cjk_query_token_char(value: char) -> bool {
    matches!(
        value as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
    )
}

fn lexical_cjk_phrase_boost(term: &str) -> f64 {
    let mut char_count = 0;
    let mut cjk_count = 0;
    for value in term.chars() {
        char_count += 1;
        if is_cjk_query_token_char(value) {
            cjk_count += 1;
        }
    }
    if char_count == 1 && cjk_count == 1 {
        return 0.2;
    }
    if char_count >= 2 && char_count == cjk_count {
        1.0 + ((char_count - 1) as f64 * 0.35).min(1.75)
    } else {
        1.0
    }
}

fn normalize_lexical_query_token(token: &str) -> Option<String> {
    if token.len() < 2 || is_lexical_stop_word(token) {
        return None;
    }

    Some(token.to_string())
}

fn is_lexical_stop_word(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "was"
            | "were"
            | "with"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_query_term_weights_include_cjk_phrase_ngrams() {
        let weights = lexical_query_term_weights("订单AI延期 风险");

        for expected in ["订", "单", "订单", "ai", "延", "期", "延期", "风险"] {
            assert!(
                weights.contains_key(expected),
                "missing expected lexical term {expected}"
            );
        }
        assert!(!weights.contains_key("单延"));
        assert!(!weights.contains_key("期风"));

        let phrase_weights = lexical_query_term_weights("订单延期风险");
        assert!(phrase_weights.contains_key("订单延期风险"));
        assert!(
            phrase_weights["订单延期风险"] > phrase_weights["订"],
            "full business phrase should carry more weight than a single character"
        );

        let person_title_weights = lexical_query_term_weights("邓工是谁");
        assert!(person_title_weights.contains_key("邓工"));
    }

    #[test]
    fn lexical_query_term_weights_include_ascii_connector_acronyms() {
        let weights = lexical_query_term_weights("IOA系统 Q&A 操作 A/B 流程 v2.0");

        for expected in ["ioa", "系统", "qa", "操作", "ab", "流程", "v20"] {
            assert!(
                weights.contains_key(expected),
                "missing expected lexical term {expected}"
            );
        }
        assert!(!weights.contains_key("q"));
        assert!(!weights.contains_key("a"));
    }

    #[test]
    fn lexical_domain_hint_score_boosts_nursing_handover_content() {
        let relevant = lexical_domain_hint_score(
            "四、交接内容：护理记录单应记录药物服用情况、身体异常状况、皮肤受压情况和管路通畅。",
            "护理交接班时，必须交接的内容有哪些？",
        );
        let unrelated = lexical_domain_hint_score(
            "无障碍设施设计应符合通道宽度和坡度要求。",
            "护理交接班时，必须交接的内容有哪些？",
        );

        assert!(relevant > 1.0);
        assert_eq!(unrelated, 0.0);
    }

    #[test]
    fn expanded_supply_prompt_only_expands_procedure_or_eldercare_signals() {
        assert!(!assistant_run_prompt_requests_expanded_supply(
            "这份资料的作者是谁"
        ));
        assert_eq!(
            assistant_run_expanded_supply_prompt("  这份资料的作者是谁  "),
            "这份资料的作者是谁"
        );

        assert!(assistant_run_prompt_requests_expanded_supply(
            "老人摔倒后怎么办"
        ));
        let fall_prompt = assistant_run_expanded_supply_prompt("老人摔倒后怎么办");
        for expected in ["老人摔倒后怎么办", "应急处置", "通知家属", "不要急于扶起"]
        {
            assert!(
                fall_prompt.contains(expected),
                "missing expanded fall prompt term {expected}"
            );
        }

        assert!(assistant_run_prompt_requests_expanded_supply(
            "长者离世后的家属对接流程"
        ));
        let death_prompt = assistant_run_expanded_supply_prompt("长者离世后的家属对接流程");
        for expected in [
            "长者离世后的家属对接流程",
            "生命体征",
            "遗物交接",
            "记录归档",
        ] {
            assert!(
                death_prompt.contains(expected),
                "missing expanded death prompt term {expected}"
            );
        }
    }

    #[test]
    fn reduced_query_helpers_strip_noise_and_keep_business_terms() {
        assert_eq!(
            assistant_run_strip_question_noise("请问这份资料里邓工是谁"),
            "邓工"
        );

        let key_terms = assistant_run_key_terms_query("请问邓工是谁 IOA系统", 20);
        for expected in ["邓工", "ioa", "系统"] {
            assert!(
                key_terms.split_whitespace().any(|term| term == expected),
                "missing key term {expected} from {key_terms}"
            );
        }
    }

    #[test]
    fn rank_query_variants_preserve_original_and_eldercare_expansions() {
        let variants = assistant_run_rank_query_variants("老人摔倒后怎么办");
        let texts = variants
            .iter()
            .map(|variant| variant.text.as_str())
            .collect::<Vec<_>>();

        assert!(texts.iter().any(|text| *text == "老人摔倒后怎么办"));
        assert!(texts.iter().any(|text| text.contains("老人摔倒后")));
        assert!(texts.iter().any(|text| text.contains("事故处理")));
        assert!(variants.iter().all(|variant| variant.norm > 0.0));
    }

    #[test]
    fn lexical_query_term_weights_extend_elder_fall_terms_without_medication_context() {
        let weights =
            lexical_query_term_weights("老人突发事件应急处置：重伤事故直接拨打120并通知家属");

        for expected in ["摔倒", "跌倒", "事故处理", "应急处置", "通知家属"] {
            assert!(
                weights.contains_key(expected),
                "missing elder-fall expansion term {expected}"
            );
        }
    }

    #[test]
    fn lexical_query_term_weights_extend_elder_death_terms() {
        let weights = lexical_query_term_weights("长者离世后通知家属并做好遗物交接");

        for expected in ["死亡", "身故", "殡仪接运", "遗物清点", "记录归档"] {
            assert!(
                weights.contains_key(expected),
                "missing elder-death expansion term {expected}"
            );
        }
    }
}
