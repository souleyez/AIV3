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
            external_channel_text_has_any(&normalized, text, &["publish", "发布", "推送", "投递"])
        }
        "external_artifact.revoke" => external_channel_text_has_any(
            &normalized,
            text,
            &["revoke", "撤回", "下线", "取消发布"],
        ),
        "external_business_action.invoke" => external_channel_text_has_any(
            &normalized,
            text,
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
        ),
        _ => false,
    }
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
    }
}
