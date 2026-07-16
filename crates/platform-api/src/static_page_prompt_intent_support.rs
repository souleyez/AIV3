use crate::external_channel_text_has_any;
use crate::prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any};

pub(crate) fn static_page_prompt_requests_explicit_redesign(prompt: &str) -> bool {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let negated_redesign = external_channel_text_has_any(
        &compact,
        prompt,
        &[
            "不要重新出图",
            "不重新出图",
            "无需重新出图",
            "不用重新出图",
            "别重新出图",
            "不要重新生成效果图",
            "不重新生成效果图",
            "无需重新生成效果图",
            "不用重新生成效果图",
            "不要重新设计",
            "不重新设计",
            "无需重新设计",
            "不用重新设计",
            "别重新设计",
            "不换风格",
            "不要换风格",
            "不用换风格",
            "donotredesign",
            "don'tredesign",
            "noredesign",
            "withoutredesign",
        ],
    );
    let strong_new_design_signal = external_channel_text_has_any(
        &compact,
        prompt,
        &[
            "全新页面",
            "换个风格",
            "换风格",
            "不喜欢这个风格",
            "不喜欢现在风格",
            "不喜欢当前风格",
            "暗黑风格",
            "深色风格",
            "暗黑背景",
            "深色背景",
            "移动端优先",
            "手机端优先",
            "适合手机端",
            "手机端展示",
            "移动端展示",
            "卡片风格",
            "卡片式",
            "从头做",
            "newversion",
            "newpage",
            "fromscratch",
        ],
    );
    if negated_redesign && !strong_new_design_signal {
        return false;
    }
    external_channel_text_has_any(
        &compact,
        prompt,
        &[
            "重新出图",
            "重新生成效果图",
            "重新设计",
            "重做一版",
            "另起一版",
            "全新页面",
            "换个风格",
            "换风格",
            "不喜欢这个风格",
            "不喜欢现在风格",
            "不喜欢当前风格",
            "不好看",
            "观感不好",
            "暗黑一点",
            "暗黑风格",
            "深色风格",
            "暗黑背景",
            "深色背景",
            "移动端优先",
            "手机端优先",
            "适合手机端",
            "手机端展示",
            "移动端展示",
            "卡片风格",
            "卡片式",
            "从头做",
            "newversion",
            "newpage",
            "redesign",
            "reimage",
            "fromscratch",
        ],
    )
}

pub(crate) fn static_page_prompt_requests_existing_artifact_revision(prompt: &str) -> bool {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    external_channel_text_has_any(
        &compact,
        prompt,
        &[
            "修复",
            "修正",
            "更正",
            "修改",
            "调整",
            "改一下",
            "改成",
            "变更",
            "小修",
            "补充",
            "新增",
            "增加",
            "删除",
            "去掉",
            "替换",
            "联动",
            "筛选",
            "不会变",
            "不变",
            "数据绑定",
            "绑定错误",
            "口径错",
            "口径不对",
            "单位错",
            "小数点",
            "bug",
            "fix",
            "revise",
            "update",
            "change",
            "correct",
        ],
    )
}

pub(crate) fn static_page_prompt_hides_template_baseline_link_for_revision(prompt: &str) -> bool {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    external_channel_text_has_any(
        &compact,
        prompt,
        &[
            "已有页面",
            "已有报表",
            "修复",
            "修正",
            "更正",
            "修改",
            "调整",
            "改一下",
            "改成",
            "变更",
            "小修",
            "删除",
            "去掉",
            "替换",
            "联动",
            "不会变",
            "不变",
            "数据绑定",
            "绑定错误",
            "口径错",
            "口径不对",
            "单位错",
            "小数点",
            "bug",
        ],
    )
}

pub(crate) fn static_page_prompt_requests_existing_artifact_delivery(prompt: &str) -> bool {
    if static_page_prompt_negates_artifact_generation(prompt) {
        return false;
    }
    if static_page_prompt_requests_explicit_redesign(prompt)
        || static_page_prompt_requests_existing_artifact_revision(prompt)
    {
        return false;
    }
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return false;
    }
    let lower = compact.to_ascii_lowercase();
    let has_existing_signal = prompt_contains_any(
        &compact,
        &[
            "已有",
            "已经",
            "已生成",
            "已发布",
            "生成过",
            "发布过",
            "做过",
            "之前",
            "昨天",
            "前面",
            "上次",
            "刚才",
            "现有",
            "历史",
            "旧版",
            "原页面",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &["existing", "previous", "already", "published", "last"],
    );
    let has_link_signal = prompt_contains_any(
        &compact,
        &[
            "链接",
            "地址",
            "页面地址",
            "页面链接",
            "产物链接",
            "原链接",
            "旧链接",
        ],
    ) || ascii_prompt_contains_any(&lower, &["link", "url", "public_url"]);
    let has_send_signal = prompt_contains_any(
        &compact,
        &["发我", "发给", "给我", "给客户", "再发", "直接发", "直接给"],
    ) || ascii_prompt_contains_any(&lower, &["send"]);
    let has_view_signal = prompt_contains_any(&compact, &["看看", "看下", "查看", "打开"])
        || ascii_prompt_contains_any(&lower, &["open", "view"]);
    let has_no_change_signal =
        prompt_contains_any(
            &compact,
            &["不用改", "不要改", "无需修改", "别改", "直接用", "复用原"],
        ) || ascii_prompt_contains_any(&lower, &["nochange", "no-change", "reuse"]);
    let has_artifact_context =
        prompt_contains_any(
            &compact,
            &[
                "报表",
                "页面",
                "静态页",
                "产物",
                "报告",
                "dashboard",
                "report",
            ],
        ) || ascii_prompt_contains_any(&lower, &["report", "page", "artifact", "dashboard"]);

    has_existing_signal
        && (has_link_signal
            || has_send_signal
            || has_no_change_signal
            || (has_view_signal && has_artifact_context))
}

pub(crate) fn static_page_prompt_requests_existing_artifact_change(prompt: &str) -> bool {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return false;
    }
    let lower = compact.to_ascii_lowercase();
    prompt_contains_any(
        &compact,
        &[
            "修复",
            "修正",
            "更正",
            "修改",
            "调整",
            "改一下",
            "改成",
            "变更",
            "小修",
            "优化",
            "补充",
            "新增",
            "增加",
            "删除",
            "去掉",
            "替换",
            "合并",
            "拆分",
            "绑定错误",
            "绑定错",
            "口径错",
            "口径不对",
            "单位错",
            "小数点",
            "必须联动",
            "不能联动",
            "不会联动",
            "无法联动",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "fix", "revise", "change", "correct", "edit", "modify", "optimize", "remove", "add",
        ],
    )
}

pub(crate) fn static_page_prompt_negates_artifact_generation(prompt: &str) -> bool {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return false;
    }
    let lower = compact.to_ascii_lowercase();
    let cancels_current_action =
        prompt_contains_any(&compact, &["不用了", "不要了", "算了", "别做了", "不做了"])
            || compact.ends_with("取消")
            || ascii_prompt_contains_any(&lower, &["nevermind", "cancelit", "stopit"]);
    let forbids_auto_publish = prompt_contains_any(
        &compact,
        &[
            "不要发布",
            "不用发布",
            "无需发布",
            "别发布",
            "不发布",
            "不要上线",
            "不用上线",
            "无需上线",
            "别上线",
            "不上线",
            "不要公开",
            "不公开",
            "只预览",
            "仅预览",
            "先预览",
            "预览即可",
            "只要预览",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "donotpublish",
            "don'tpublish",
            "neverpublish",
            "donotmakepublic",
            "previewonly",
            "onlypreview",
        ],
    );
    cancels_current_action
        || forbids_auto_publish
        || prompt_contains_any(
            &compact,
            &[
                "不要生成报表",
                "不要生成报告",
                "不要生成页面",
                "不要生成静态页",
                "不要生成看板",
                "不要生成图表",
                "不生成报表",
                "不生成报告",
                "不生成页面",
                "不用生成报表",
                "不用生成页面",
                "无需生成报表",
                "无需生成页面",
                "别生成报表",
                "别生成页面",
                "不要创建报表",
                "不要创建页面",
                "不要制作报表",
                "不要制作页面",
                "不要输出报表",
                "不要输出页面",
                "不要出报表",
                "不要出页面",
                "不要做报表",
                "不要做页面",
                "不要返回历史页面链接",
                "不要返回页面链接",
                "不要返回报表链接",
                "不要返回产物链接",
                "不要返回旧链接",
                "不要返回历史链接",
                "不要返回之前生成过的页面链接",
                "不要返回之前生成过的报表链接",
                "不要返回之前的页面链接",
                "不要返回之前的报表链接",
                "不要发历史页面链接",
                "不要发页面链接",
                "不要发报表链接",
                "不要发旧链接",
                "不要发之前生成过的页面链接",
                "不要发之前生成过的报表链接",
                "不要发之前的页面链接",
                "不要发之前的报表链接",
                "不要给历史页面链接",
                "不要给页面链接",
                "不要给报表链接",
                "不要提供历史页面链接",
                "不要提供页面链接",
                "不要提供报表链接",
                "不返回历史页面链接",
                "不返回页面链接",
                "不返回报表链接",
                "别返回历史页面链接",
                "别返回页面链接",
                "别发历史页面链接",
                "别发页面链接",
                "不是要生成报表",
                "不是要生成页面",
                "不是让你生成报表",
                "不是让你生成页面",
            ],
        )
        || ascii_prompt_contains_any(
            &lower,
            &[
                "donotgeneratereport",
                "donotgeneratepage",
                "don'tgeneratereport",
                "don'tgeneratepage",
                "noreportgeneration",
                "nopagegeneration",
                "donotreturnreportlink",
                "donotreturnpagelink",
                "donotreturnpreviouslink",
                "don'treturnreportlink",
                "don'treturnpagelink",
                "noreportlink",
                "nopagelink",
            ],
        )
}

pub(crate) fn static_page_prompt_allows_default_template_reuse(prompt: &str) -> bool {
    if static_page_prompt_requests_explicit_redesign(prompt)
        || static_page_prompt_requests_existing_artifact_change(prompt)
        || static_page_prompt_negates_artifact_generation(prompt)
    {
        return false;
    }
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return false;
    }
    let lower = compact.to_ascii_lowercase();
    let has_artifact_context =
        prompt_contains_any(
            &compact,
            &[
                "报表",
                "页面",
                "静态页",
                "看板",
                "仪表盘",
                "可视化",
                "图表",
                "报告",
                "月报",
                "经营分析",
            ],
        ) || ascii_prompt_contains_any(&lower, &["report", "page", "dashboard", "visualization"]);
    let has_use_or_view_signal = prompt_contains_any(
        &compact,
        &[
            "生成",
            "输出",
            "创建",
            "制作",
            "做个",
            "做一个",
            "做一下",
            "刷新",
            "更新",
            "看看",
            "看下",
            "查看",
            "打开",
            "发我",
            "发给",
            "给我",
            "分析",
            "复盘",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "generate", "create", "make", "build", "refresh", "update", "view", "open", "send",
            "analyze", "analyse",
        ],
    );

    has_artifact_context && has_use_or_view_signal
}

pub(crate) fn static_page_prompt_allows_stable_artifact_reuse(prompt: &str) -> bool {
    static_page_prompt_requests_existing_artifact_delivery(prompt)
        || static_page_prompt_allows_default_template_reuse(prompt)
}

pub(crate) fn static_page_stable_artifact_reuse_reason(prompt: &str) -> &'static str {
    if static_page_prompt_requests_existing_artifact_delivery(prompt) {
        "existing_artifact_delivery_request"
    } else {
        "default_dataset_template_reuse"
    }
}

pub(crate) fn prompt_touches_current_static_page_artifact(prompt: &str) -> bool {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return false;
    }
    let lower_prompt = prompt.to_ascii_lowercase();
    [
        "静态页",
        "静态页面",
        "页面规划",
        "一页",
        "生成页面",
        "落地页",
        "模块",
        "效果图",
        "出图",
        "标题",
        "文案",
        "内容",
        "数据",
        "图表",
        "布局",
        "调整",
        "修改",
        "改",
        "换",
        "确认",
        "导出",
        "继续",
        "接着",
        "下一步",
        "刚才",
        "上面",
        "之前",
        "这个",
        "那版",
        "草稿",
        "柱状图",
        "折线图",
        "环图",
        "看板",
    ]
    .iter()
    .any(|hint| prompt.contains(hint))
        || ["dashboard", "chart", "kpi", "export"]
            .iter()
            .any(|hint| lower_prompt.contains(hint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_redesign_respects_negation_and_strong_new_style_signals() {
        assert!(!static_page_prompt_requests_explicit_redesign(
            "把标题改一下"
        ));
        assert!(!static_page_prompt_requests_explicit_redesign(
            "在原报表基础上修改标题，不要重新设计"
        ));
        assert!(!static_page_prompt_requests_explicit_redesign(
            "Update this report, do not redesign"
        ));
        assert!(static_page_prompt_requests_explicit_redesign(
            "重新设计一版全新页面"
        ));
        assert!(static_page_prompt_requests_explicit_redesign(
            "我不喜欢这个风格的报表，最好暗黑一点的背景，并且适合手机端展示"
        ));
    }

    #[test]
    fn revision_and_hidden_baseline_link_intents_keep_existing_semantics() {
        assert!(static_page_prompt_requests_existing_artifact_revision(
            "修复近7日销售，切换区域和门店后需要跟着变化"
        ));
        assert!(static_page_prompt_requests_existing_artifact_revision(
            "我临时上传了合同，帮新百经营报表补充门店面积和坪效。"
        ));
        assert!(
            !static_page_prompt_hides_template_baseline_link_for_revision(
                "我临时上传了合同，帮新百经营报表补充门店面积和坪效。"
            )
        );
        assert!(
            static_page_prompt_hides_template_baseline_link_for_revision(
                "修复近7日销售，切换区域和门店后需要跟着变化"
            )
        );
        assert!(!static_page_prompt_requests_existing_artifact_revision(
            "把之前生成过的报表链接再发我一下"
        ));
    }

    #[test]
    fn existing_artifact_delivery_requires_existing_and_delivery_signals() {
        assert!(static_page_prompt_requests_existing_artifact_delivery(
            "把之前生成过的报表链接再发我一下"
        ));
        assert!(static_page_prompt_requests_existing_artifact_delivery(
            "把昨天/之前生成的新百报表链接发我一下"
        ));
        assert!(!static_page_prompt_requests_existing_artifact_delivery(
            "随便生成一个报表我看看"
        ));
        assert!(!static_page_prompt_requests_existing_artifact_delivery(
            "生成新百经营分析月报，按当前数据刷新并保留分店筛选"
        ));
        assert!(!static_page_prompt_requests_existing_artifact_delivery(
            "修复近7日销售，切换区域和门店后需要跟着变化"
        ));
        assert!(!static_page_prompt_requests_existing_artifact_delivery(
            "请直接回答，不要返回历史页面链接"
        ));
        assert!(!static_page_prompt_requests_existing_artifact_delivery(
            "普通问答测速，不要发之前生成过的报表链接"
        ));
        assert!(static_page_prompt_requests_existing_artifact_delivery(
            "不要重新生成，直接把之前生成过的报表链接再发我一下"
        ));
    }

    #[test]
    fn stable_reuse_allows_default_template_but_blocks_changes_and_redesign() {
        assert!(!static_page_prompt_allows_stable_artifact_reuse(
            "把标题改一下"
        ));
        assert!(!static_page_prompt_allows_stable_artifact_reuse(
            "修复近7日销售，切换区域和门店后需要跟着变化"
        ));
        assert!(!static_page_prompt_allows_stable_artifact_reuse(
            "重新设计一版暗黑移动端报表"
        ));
        assert!(!static_page_prompt_allows_stable_artifact_reuse(
            "请用两句话回答：V3 第三方普通问答测速，请直接回答不要生成报表。"
        ));
        assert!(!static_page_prompt_allows_stable_artifact_reuse(
            "请用两句话回答：这是第三方普通问答速度 smoke，不要返回历史页面链接。"
        ));
        assert!(!static_page_prompt_allows_stable_artifact_reuse(
            "先普通回答这个报表问题，不要生成页面"
        ));
        assert!(static_page_prompt_allows_stable_artifact_reuse(
            "看看最新的门店取高报表"
        ));
        assert!(static_page_prompt_allows_stable_artifact_reuse(
            "随便生成一个报表我看看"
        ));
        assert!(static_page_prompt_allows_stable_artifact_reuse(
            "生成新百经营分析月报，按当前数据刷新并保留分店筛选"
        ));
        assert_eq!(
            static_page_stable_artifact_reuse_reason("看看最新的门店取高报表"),
            "default_dataset_template_reuse"
        );
        assert_eq!(
            static_page_stable_artifact_reuse_reason("把之前生成过的报表链接再发我一下"),
            "existing_artifact_delivery_request"
        );
    }

    #[test]
    fn current_static_page_artifact_touch_intent_keeps_existing_hints() {
        assert!(!prompt_touches_current_static_page_artifact(""));
        assert!(!prompt_touches_current_static_page_artifact("普通问答测速"));
        assert!(prompt_touches_current_static_page_artifact(
            "把标题和图表布局调整一下"
        ));
        assert!(prompt_touches_current_static_page_artifact(
            "继续用刚才那版草稿导出"
        ));
        assert!(prompt_touches_current_static_page_artifact(
            "refresh the dashboard KPI chart export"
        ));
    }
}
