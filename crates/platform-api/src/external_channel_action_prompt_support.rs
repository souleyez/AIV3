pub(crate) fn external_channel_prompt_may_need_planned_action(prompt: &str) -> bool {
    let normalized = prompt.to_ascii_lowercase();
    let text = prompt.trim();
    external_channel_text_has_any(
        &normalized,
        text,
        &[
            "web search",
            "search web",
            "latest",
            "publish",
            "revoke",
            "dispatch",
            "callback",
            "artifact status",
            "delivery status",
            "publish status",
            "external action status",
            "external artifact status",
            "产物状态",
            "投递状态",
            "发布状态",
            "外部动作状态",
            "外部产物状态",
            "发布",
            "撤回",
            "下线",
            "派发",
            "回调",
            "执行第三方",
            "执行动作",
            "业务动作",
            "发起审批",
            "提交审批",
            "创建工单",
            "联网搜索",
            "网页搜索",
            "最新",
            "实时",
        ],
    )
}

pub(crate) fn external_channel_prompt_allows_external_action(
    prompt: &str,
    action_type: &str,
) -> bool {
    let normalized = prompt.to_ascii_lowercase();
    let text = prompt.trim();
    match action_type {
        "external_artifact.status" => external_channel_text_has_any(
            &normalized,
            text,
            &[
                "artifact status",
                "delivery status",
                "publish status",
                "external artifact status",
                "产物状态",
                "投递状态",
                "发布状态",
                "外部产物状态",
            ],
        ),
        "external_artifact.publish" => {
            !external_channel_text_has_any(
                &normalized,
                text,
                &["revoke", "撤回", "下线", "取消发布"],
            ) && crate::external_channel_prompt_explicitly_authorizes_capability_action(
                prompt,
                &["publish", "发布", "推送", "投递"],
                &[
                    "artifact",
                    "report",
                    "page",
                    "dashboard",
                    "chart",
                    "file",
                    "link",
                    "message",
                    "version",
                    "产物",
                    "报表",
                    "报告",
                    "页面",
                    "看板",
                    "图表",
                    "文件",
                    "链接",
                    "消息",
                    "版本",
                ],
            )
        }
        "external_artifact.revoke" => {
            crate::external_channel_prompt_explicitly_authorizes_capability_action(
                prompt,
                &["revoke", "撤回", "下线", "取消发布"],
                &[
                    "artifact",
                    "report",
                    "page",
                    "dashboard",
                    "chart",
                    "file",
                    "link",
                    "version",
                    "产物",
                    "报表",
                    "报告",
                    "页面",
                    "看板",
                    "图表",
                    "文件",
                    "链接",
                    "版本",
                ],
            )
        }
        "external_business_action.invoke" => {
            crate::external_channel_prompt_explicitly_authorizes_capability_action(
                prompt,
                &[
                    "dispatch",
                    "invoke",
                    "callback",
                    "执行第三方",
                    "执行动作",
                    "业务动作",
                    "发起审批",
                    "提交审批",
                    "创建工单",
                ],
                &[],
            )
        }
        _ => false,
    }
}

pub(crate) fn external_channel_phrase_term_position(
    phrase: &str,
    compact: &str,
    term: &str,
) -> Option<usize> {
    if !term.is_ascii() {
        return compact.find(term);
    }
    let term = term.to_ascii_lowercase();
    if term
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return phrase.match_indices(&term).find_map(|(index, _)| {
            let before = phrase[..index].chars().next_back();
            let after = phrase[index + term.len()..].chars().next();
            let bounded_before = !before
                .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
            let bounded_after = !after
                .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
            (bounded_before && bounded_after).then_some(index)
        });
    }
    phrase.find(&term)
}

fn external_channel_static_action_position(
    phrase: &str,
    compact: &str,
    action: &str,
) -> Option<usize> {
    if action.is_ascii() {
        return external_channel_phrase_term_position(phrase, compact, action);
    }
    let invalid_suffixes: &[&str] = match action {
        "生成" => &["式"],
        "发布" => &["会"],
        "更新" => &["世"],
        "输出" => &["轴"],
        "制作" => &["人"],
        _ => &[],
    };
    compact.match_indices(action).find_map(|(position, _)| {
        let tail = &compact[position + action.len()..];
        (!invalid_suffixes
            .iter()
            .any(|suffix| tail.starts_with(suffix)))
        .then_some(position)
    })
}

pub(crate) fn external_channel_prompt_explicitly_authorizes_static_page_action(
    prompt: &str,
) -> bool {
    if crate::static_page_prompt_negates_artifact_generation(prompt) {
        return false;
    }
    let trimmed_prompt = prompt.trim_end();
    if trimmed_prompt.ends_with('?')
        || trimmed_prompt.ends_with('？')
        || ["还是", "行不行", "要不要"]
            .iter()
            .any(|marker| prompt.contains(marker))
    {
        return false;
    }

    let mut phrases = prompt.to_ascii_lowercase();
    for separator in [
        " and then ",
        " then ",
        " and ",
        "同时",
        "然后",
        "并且",
        "并",
        "，",
        ",",
        "。",
        ".",
        "；",
        ";",
        "！",
        "!",
        "？",
        "?",
    ] {
        phrases = phrases.replace(separator, "\n");
    }

    phrases.lines().any(|phrase| {
        let compact = phrase
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        if compact.is_empty() {
            return false;
        }

        let artifact_targets = [
            "静态页",
            "静态页面",
            "报表页",
            "可视化报表",
            "经营分析报表",
            "经营分析页",
            "报表",
            "报告",
            "月报",
            "周报",
            "日报",
            "看板",
            "仪表盘",
            "大屏",
            "页面",
            "网页",
            "网站",
            "图表",
            "dashboard",
            "report",
            "page",
            "webpage",
            "html",
            "artifact",
            "chart",
        ];
        let explicit_actions = [
            "重新生成",
            "生成",
            "创建",
            "制作",
            "做出来",
            "做成",
            "做一个",
            "做一份",
            "做个",
            "输出",
            "发布",
            "上线",
            "渲染",
            "导出",
            "重新设计",
            "重做",
            "改版",
            "修改",
            "修复",
            "更新",
            "调整",
            "补充",
            "增加",
            "添加",
            "替换",
            "出页面",
            "出报表",
            "出报告",
            "出看板",
            "create",
            "generate",
            "build",
            "make",
            "publish",
            "render",
            "export",
            "redesign",
            "revise",
            "modify",
            "update",
        ];
        let question_or_status_markers = [
            "查看",
            "看看",
            "状态",
            "记录",
            "进度",
            "历史",
            "结果",
            "说明",
            "建议",
            "计划",
            "方案",
            "讨论",
            "评估",
            "研究",
            "考虑",
            "梳理",
            "步骤",
            "规则",
            "日志",
            "详情",
            "什么",
            "为什么",
            "为何",
            "怎么",
            "如何",
            "是否",
            "能不能",
            "可不可以",
            "原因",
            "含义",
            "是什么意思",
            "失败",
            "why",
            "how",
            "what",
            "whether",
            "failed",
            "failure",
            "show",
            "view",
            "open",
            "existing",
            "previous",
            "history",
        ];
        let historical_action_markers = [
            "已生成",
            "已经生成",
            "生成过",
            "生成的",
            "之前生成",
            "先前生成",
            "去年生成",
            "曾经生成",
            "曾生成",
            "已创建",
            "创建过",
            "创建的",
            "已发布",
            "发布过",
            "发布的",
            "上次生成",
            "上次创建",
            "上次发布",
            "上次更新",
            "alreadygenerated",
            "previouslygenerated",
            "generatedreport",
            "generatedpage",
        ];
        let negation_markers = [
            "不要",
            "不用",
            "无需",
            "无须",
            "不必",
            "别",
            "禁止",
            "不是要",
            "不是让你",
            "不",
            "donot",
            "don't",
            "dont",
            "never",
            "without",
            "not",
        ];

        let Some(action_position) = explicit_actions
            .iter()
            .filter_map(|needle| external_channel_static_action_position(phrase, &compact, needle))
            .min()
        else {
            return false;
        };
        if !artifact_targets
            .iter()
            .any(|needle| external_channel_phrase_term_position(phrase, &compact, needle).is_some())
        {
            return false;
        }
        if question_or_status_markers
            .iter()
            .any(|needle| external_channel_phrase_term_position(phrase, &compact, needle).is_some())
            || historical_action_markers.iter().any(|needle| {
                external_channel_phrase_term_position(phrase, &compact, needle).is_some()
            })
            || external_channel_phrase_is_action_completion_status(&compact, action_position)
        {
            return false;
        }
        if ["有哪些", "howto", "tutorial"]
            .iter()
            .any(|needle| external_channel_phrase_term_position(phrase, &compact, needle).is_some())
            || ["方法", "教程", "做法", "流程", "示例", "例子"]
                .iter()
                .filter_map(|needle| {
                    external_channel_phrase_term_position(phrase, &compact, needle)
                })
                .any(|position| position >= action_position)
        {
            return false;
        }
        if negation_markers
            .iter()
            .filter_map(|needle| external_channel_phrase_term_position(phrase, &compact, needle))
            .any(|position| position <= action_position)
        {
            return false;
        }

        let explicitly_commanded = [
            "请",
            "帮我",
            "麻烦",
            "需要",
            "我要",
            "我想",
            "我们要",
            "我们想",
            "想要",
            "把",
            "给我",
            "开始",
            "重新",
            "立即",
            "现在",
            "按",
            "基于",
            "根据",
            "引用",
            "总结",
            "please",
            "iwant",
            "weneed",
            "let's",
            "lets",
        ]
        .iter()
        .any(|prefix| compact.starts_with(prefix))
            || (compact.starts_with("帮") && !compact.starts_with("帮助"))
            || compact.starts_with("将");

        action_position == 0 || explicitly_commanded
    })
}

pub(crate) fn external_channel_phrase_is_action_completion_status(
    compact: &str,
    action_position: usize,
) -> bool {
    let action_tail = compact.get(action_position..).unwrap_or(compact);
    if ["吗", "么", "是否", "有没有", "是不是"]
        .iter()
        .any(|marker| action_tail.contains(marker))
    {
        return true;
    }

    [
        "已完成",
        "已经完成",
        "完成",
        "完成了",
        "完成没",
        "已成功",
        "已经成功",
        "成功",
        "成功了",
        "成功没",
        "done",
        "completed",
        "finished",
        "时间",
    ]
    .iter()
    .any(|marker| action_tail.ends_with(marker))
}

pub(crate) fn external_channel_text_has_any(
    normalized_ascii: &str,
    original: &str,
    needles: &[&str],
) -> bool {
    needles.iter().any(|needle| {
        if needle.is_ascii() {
            normalized_ascii.contains(&needle.to_ascii_lowercase())
        } else {
            original.contains(needle)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_has_any_matches_ascii_case_insensitively_and_cjk_directly() {
        assert!(external_channel_text_has_any(
            "please publish the report",
            "Please PUBLISH the report",
            &["publish"]
        ));
        assert!(external_channel_text_has_any(
            "ignored",
            "请发布这个产物",
            &["发布"]
        ));
        assert!(!external_channel_text_has_any(
            "please summarize",
            "请总结资料",
            &["publish", "发布"]
        ));
    }

    #[test]
    fn planned_action_gate_allows_only_explicit_action_or_search_intents() {
        assert!(!external_channel_prompt_may_need_planned_action(
            "1+1等于几"
        ));
        assert!(!external_channel_prompt_may_need_planned_action(
            "帮我总结采购审批制度，并指出风险。"
        ));
        assert!(external_channel_prompt_may_need_planned_action(
            "请查询第三方产物状态"
        ));
        assert!(external_channel_prompt_may_need_planned_action(
            "web search the latest policy"
        ));
        assert!(external_channel_prompt_may_need_planned_action(
            "联网搜索一下最新消息"
        ));
    }

    #[test]
    fn planned_action_gate_rejects_generic_status_without_external_scope() {
        assert!(!external_channel_prompt_may_need_planned_action("status"));
        assert!(!external_channel_prompt_may_need_planned_action("查询状态"));
        assert!(!external_channel_prompt_allows_external_action(
            "status",
            "external_artifact.status"
        ));
        assert!(!external_channel_prompt_allows_external_action(
            "查询状态",
            "external_artifact.status"
        ));
    }

    #[test]
    fn external_action_allow_list_is_action_specific() {
        assert!(external_channel_prompt_allows_external_action(
            "请查询第三方产物状态",
            "external_artifact.status"
        ));
        assert!(external_channel_prompt_allows_external_action(
            "请发布这个报表产物",
            "external_artifact.publish"
        ));
        assert!(external_channel_prompt_allows_external_action(
            "请撤回这个报表产物",
            "external_artifact.revoke"
        ));
        assert!(external_channel_prompt_allows_external_action(
            "请执行第三方业务动作并回调结果",
            "external_business_action.invoke"
        ));
        assert!(!external_channel_prompt_allows_external_action(
            "请发布这个报表产物",
            "external_business_action.invoke"
        ));
        assert!(!external_channel_prompt_allows_external_action(
            "请执行第三方业务动作",
            "external_artifact.publish"
        ));
        assert!(!external_channel_prompt_allows_external_action(
            "请查询第三方产物状态",
            "unknown.action"
        ));
        for prompt in [
            "报告发布状态",
            "不要发布这个报表",
            "上次发布的报表",
            "报表发布时间",
            "发布报表完成了吗",
            "请取消发布这个报表",
            "发布报表不用了",
            "发布报表取消",
            "请给我报告发布计划",
        ] {
            assert!(
                !external_channel_prompt_allows_external_action(
                    prompt,
                    "external_artifact.publish"
                ),
                "read-only, historical, negated, or revoke wording must not authorize publish: {prompt}"
            );
        }
        assert!(external_channel_prompt_allows_external_action(
            "请取消发布这个报表",
            "external_artifact.revoke"
        ));
    }

    #[test]
    fn static_page_action_authorization_requires_action_and_artifact_in_same_phrase() {
        for prompt in [
            "生成经营健康度报表",
            "按模板把新百经营月报做出来",
            "帮经营报表补充门店面积",
            "把这个看板导出成页面",
            "先解释销售数据，然后生成经营分析报表",
            "Generate a dashboard and explain the risk evidence",
            "引用现有报告内容生成经营看板",
            "总结以上数据生成报告",
            "按模板生成经营分析页",
        ] {
            assert!(
                external_channel_prompt_explicitly_authorizes_static_page_action(prompt),
                "prompt should explicitly authorize a static-page action: {prompt}"
            );
        }
    }

    #[test]
    fn static_page_action_authorization_keeps_report_reference_questions_read_only() {
        for prompt in [
            "解释销售缺口最大的门店，同时引用报告里的风险描述。",
            "请看当前销售报告",
            "经营健康度报表",
            "门店经营看板",
            "引用报告里的风险描述",
            "报告中提到生成页面的原因是什么",
            "解释报告中为什么建议生成页面",
            "报告输出是什么",
            "页面生成为什么失败",
            "删除这份报表",
            "不要重新生成报表，只解释现有报告",
            "不要生成新的页面",
            "don't build a new dashboard",
            "修改合同模板",
            "更新 Excel 模板的列",
            "查看已生成的报告",
            "查看报表更新记录",
            "报表发布状态",
            "报告更新说明",
            "报表修改建议",
            "报告发布计划",
            "页面更新方案",
            "去年生成过一份经营报告",
            "系统曾经生成经营报告",
            "系统会生成经营报告",
            "经营报告由系统生成",
            "如果生成经营报告，需要多久",
            "若生成报表会有什么影响",
            "讨论生成报告",
            "请讨论生成报告",
            "评估生成报表的风险",
            "请评估生成报表的风险",
            "研究生成报告的可行性",
            "考虑生成经营看板",
            "梳理生成报表的流程",
            "生成报表完成了吗",
            "生成报表已完成",
            "生成报表是否完成",
            "报表生成完成时间",
            "报表发布时间",
            "上次生成报表",
            "生成报表不用了",
            "生成报表取消",
            "请生成报表，但不要发布",
            "请生成报表但不要上线",
            "请生成报表，只预览",
            "生成报表的方法",
            "生成报表教程",
            "有哪些生成报表的方法",
            "生成报表示例",
            "生成报表？",
            "生成报表还是不生成",
            "生成报表行不行",
            "published report",
            "latest updated report",
            "show the exported report",
            "view the rendered dashboard",
            "生成式 AI 报告",
            "发布会报告",
            "更新世地质报告",
            "输出轴检测报告",
            "制作人报告",
            "不要生成报表，只解释现有报告",
            "Explain why the report suggests generating a page",
        ] {
            assert!(
                !external_channel_prompt_explicitly_authorizes_static_page_action(prompt),
                "read-only report question must not authorize a static-page action: {prompt}"
            );
        }
    }

    #[test]
    fn static_page_action_authorization_keeps_future_completion_commands_positive() {
        for prompt in [
            "请生成报表，完成后通知我",
            "请生成报表完成后通知我",
            "请发布已完成的经营报表",
            "publish the report",
            "update the dashboard",
            "export the report",
        ] {
            assert!(
                external_channel_prompt_explicitly_authorizes_static_page_action(prompt),
                "future completion command should remain authorized: {prompt}"
            );
        }
    }
}
