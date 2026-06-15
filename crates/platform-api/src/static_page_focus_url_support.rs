use crate::{
    codex_host_fixed_task_public_artifact_url_allowed, static_page_template_text_contains_any,
};

pub(crate) fn static_page_public_url_with_prompt_focus(public_url: &str, prompt: &str) -> String {
    let Some(focus) = static_page_prompt_focus_query_value(prompt) else {
        return public_url.to_string();
    };
    static_page_public_url_with_focus_label(public_url, focus)
}

pub(crate) fn static_page_prompt_focus_query_value(prompt: &str) -> Option<&'static str> {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let lower = compact.to_ascii_lowercase();
    if static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "取高",
            "高分成",
            "提成",
            "分成线",
            "取高线",
            "超溢",
            "缺口",
            "机会",
            "达线",
            "触发取高",
            "机会门店",
            "助推",
            "需助推",
            "需要助推",
            "助推门店",
            "高预警",
            "中预警",
            "租金",
            "rent",
            "commission",
            "takehigh",
        ],
    ) {
        return Some("取高机会");
    }
    if static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "低活跃",
            "不活跃",
            "零销售",
            "无销售",
            "连续无销售",
            "低销售",
            "客流下降",
            "客流降低",
            "同比下降",
            "客流预警",
            "异常",
            "inactive",
            "低活跃品牌",
        ],
    ) {
        return Some("低活跃风险");
    }
    if static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "风险店铺",
            "风险门店",
            "风险品牌",
            "风险提示",
            "风险识别",
            "风险识别系统",
            "风险",
            "预警",
            "高风险",
            "risk",
        ],
    ) {
        return Some("风险店铺");
    }
    if static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "合同面积",
            "门店面积",
            "面积数据",
            "坪效",
            "客流统计",
            "客流数据",
            "客流同比",
            "客流月同比",
            "客流年同比",
            "租售比",
            "经营健康",
            "健康度",
            "评分",
            "经营评分",
            "销售趋势",
            "月度销售趋势",
            "收入趋势",
            "收入总额",
            "收入同比",
            "计划完成",
        ],
    ) {
        return Some("经营总览");
    }
    if static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "明细",
            "品牌",
            "品牌店",
            "店铺客户",
            "客户名单",
            "合同",
            "detail",
        ],
    ) {
        return Some("品牌明细");
    }
    if static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "品类",
            "品类分析",
            "业态",
            "类别",
            "结构",
            "占比",
            "category",
        ],
    ) {
        return Some("品类业态");
    }
    if static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "经营总览",
            "经营状况",
            "经营情况",
            "经营状态",
            "总览",
            "overview",
            "health",
            "经营健康度",
        ],
    ) {
        return Some("经营总览");
    }
    None
}

pub(crate) fn static_page_public_url_with_focus_label(public_url: &str, focus: &str) -> String {
    let focus = focus.trim();
    if focus.is_empty() {
        return public_url.to_string();
    }
    let Ok(mut url) = reqwest::Url::parse(public_url) else {
        return public_url.to_string();
    };
    if !codex_host_fixed_task_public_artifact_url_allowed(url.as_str()) {
        return public_url.to_string();
    }
    let existing_pairs = url
        .query_pairs()
        .filter(|(key, _)| key != "focus")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in existing_pairs {
            pairs.append_pair(&key, &value);
        }
        pairs.append_pair("focus", focus);
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/index.html";

    #[test]
    fn prompt_focus_url_adds_or_replaces_focus_query() {
        assert_eq!(
            static_page_public_url_with_prompt_focus(PUBLIC_URL, "看一下风险店铺排行"),
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/index.html?focus=%E9%A3%8E%E9%99%A9%E5%BA%97%E9%93%BA"
        );
        assert_eq!(
            static_page_public_url_with_prompt_focus(
                "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/index.html?foo=bar&focus=old",
                "最近取高机会",
            ),
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/index.html?foo=bar&focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A"
        );
    }

    #[test]
    fn prompt_focus_url_keeps_unfocused_or_disallowed_url_unchanged() {
        assert_eq!(
            static_page_public_url_with_prompt_focus(PUBLIC_URL, "生成经营月报"),
            PUBLIC_URL
        );
        assert_eq!(
            static_page_public_url_with_prompt_focus(
                "https://example.com/generated-artifacts/database-static-pages/xinbai/index.html",
                "最近取高机会",
            ),
            "https://example.com/generated-artifacts/database-static-pages/xinbai/index.html"
        );
    }

    #[test]
    fn prompt_focus_detection_covers_default_xinbai_modules() {
        for (prompt, expected_focus) in [
            ("低活跃品牌有哪些", "低活跃风险"),
            ("展示品牌店铺客户明细", "品牌明细"),
            ("按品类业态看一下销售结构", "品类业态"),
            ("取高预警门店有哪些", "取高机会"),
            ("客流下降风险门店", "低活跃风险"),
            ("经营健康度评分表", "经营总览"),
            ("经营状况", "经营总览"),
            ("看经营总览", "经营总览"),
            ("看看新街口店经营风险", "风险店铺"),
            ("哪些需要助推的门店", "取高机会"),
            ("销售缺口统计一下，哪些门店需要助推", "取高机会"),
            ("看月度销售趋势", "经营总览"),
            ("客流降低预警", "低活跃风险"),
            (
                "按这个模板把新百经营月报做出来，重点放取高机会和风险门店。",
                "取高机会",
            ),
            (
                "我临时上传了合同，帮新百经营报表补充门店面积和坪效。",
                "经营总览",
            ),
            ("上传了客流统计，帮经营健康度报表增加客流同比。", "经营总览"),
            ("给我一份品类分析报表", "品类业态"),
            ("风险识别系统有哪些门店需要关注？", "风险店铺"),
        ] {
            assert_eq!(
                static_page_prompt_focus_query_value(prompt),
                Some(expected_focus)
            );
        }
    }
}
