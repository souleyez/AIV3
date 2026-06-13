use serde_json::Value;

use crate::assistant_run_detail_support::assistant_run_detail_target_count;

pub(crate) fn assistant_run_evidence_supplied_count(evidence_state: &Value) -> usize {
    evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

pub(crate) fn assistant_run_evidence_status_label(evidence_state: &Value) -> String {
    let status = evidence_state
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match status {
        "supplied" => {
            let supplied_count = assistant_run_evidence_supplied_count(evidence_state);
            let detail_target_count = assistant_run_detail_target_count(evidence_state);
            let fallback_supply_count = evidence_state
                .get("fallback_supply_count")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if fallback_supply_count > 0 {
                if detail_target_count > 0 {
                    return format!(
                        "已供料 {supplied_count} 条，其中 {fallback_supply_count} 条来自可见文档切片兜底，建议深读 {detail_target_count} 份文档"
                    );
                }
                return format!(
                    "已供料 {supplied_count} 条，其中 {fallback_supply_count} 条来自可见文档切片兜底"
                );
            }
            if detail_target_count > 0 {
                format!("已检索 {supplied_count} 条供料项，建议深读 {detail_target_count} 份文档")
            } else {
                format!("已检索 {supplied_count} 条供料项")
            }
        }
        "empty" => "已请求供料，但暂未检索到可用内容".to_string(),
        "not_requested" => "未请求数据集供料".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn assistant_run_placeholder_user_message(
    is_continue: bool,
    evidence_state: &Value,
) -> String {
    let status_label = assistant_run_evidence_status_label(evidence_state);
    let supply_note = if status_label.starts_with("未请求") {
        "当前按普通聊天处理；DataMax 上下文只作为附加能力，不会限制通用问答。"
    } else if status_label.starts_with("已请求") {
        "本轮没有可引用的 DataMax 供料；涉及 DataMax 数据、文档、权限或产物状态时，应说明“当前不可见/未供料”。"
    } else if status_label.starts_with("已供料") || status_label.starts_with("已检索") {
        "已有可见供料时，正式模型回答会优先参考供料，并区分供料事实和通用判断。"
    } else {
        "正式模型回答会遵守 DataMax 可见供料和权限边界；缺少证据时不编造。"
    };
    let opening = if is_continue {
        "我已收到继续指令。"
    } else {
        "我已收到你的问题。"
    };
    format!(
        "{opening}当前环境使用占位运行时，未调用外部模型生成最终自然回答；真实模型接入后会直接面向用户作答，内部运行信息只保留在运行详情中，不作为回答正文展示。\n\n{supply_note}"
    )
}

pub(crate) fn assistant_run_evidence_trail_label(evidence_state: &Value) -> &'static str {
    match evidence_state
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
    {
        "not_requested" => "判断无需数据集供料",
        "empty" => "尝试供料但无结果",
        _ => "检索供料证据",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn supplied_count_reads_only_supplied_items_array() {
        assert_eq!(
            assistant_run_evidence_supplied_count(&json!({
                "supplied_items": [{}, {}, {}]
            })),
            3
        );
        assert_eq!(
            assistant_run_evidence_supplied_count(&json!({
                "supplied_items": "3"
            })),
            0
        );
    }

    #[test]
    fn status_label_summarizes_supplied_detail_and_fallback_counts() {
        let label = assistant_run_evidence_status_label(&json!({
            "status": "supplied",
            "supplied_items": [{}, {}, {}],
            "detail_targets": [{}, {}],
            "fallback_supply_count": 1
        }));

        assert_eq!(
            label,
            "已供料 3 条，其中 1 条来自可见文档切片兜底，建议深读 2 份文档"
        );
    }

    #[test]
    fn status_label_keeps_empty_not_requested_and_unknown_shape() {
        assert_eq!(
            assistant_run_evidence_status_label(&json!({"status": "empty"})),
            "已请求供料，但暂未检索到可用内容"
        );
        assert_eq!(
            assistant_run_evidence_status_label(&json!({"status": "not_requested"})),
            "未请求数据集供料"
        );
        assert_eq!(
            assistant_run_evidence_status_label(&json!({"status": "custom"})),
            "custom"
        );
    }

    #[test]
    fn placeholder_message_explains_regular_chat_without_supply() {
        let message =
            assistant_run_placeholder_user_message(false, &json!({"status": "not_requested"}));

        assert!(message.starts_with("我已收到你的问题。当前环境使用占位运行时"));
        assert!(message.contains("当前按普通聊天处理"));
        assert!(message.contains("不会限制通用问答"));
    }

    #[test]
    fn placeholder_message_explains_continue_and_empty_supply() {
        let message = assistant_run_placeholder_user_message(true, &json!({"status": "empty"}));

        assert!(message.starts_with("我已收到继续指令。当前环境使用占位运行时"));
        assert!(message.contains("本轮没有可引用的 DataMax 供料"));
        assert!(message.contains("当前不可见/未供料"));
    }

    #[test]
    fn placeholder_message_explains_supplied_evidence_boundary() {
        let message = assistant_run_placeholder_user_message(
            false,
            &json!({
                "status": "supplied",
                "supplied_items": [{}]
            }),
        );

        assert!(message.contains("已有可见供料时"));
        assert!(message.contains("区分供料事实和通用判断"));
    }

    #[test]
    fn evidence_trail_label_matches_supply_status() {
        assert_eq!(
            assistant_run_evidence_trail_label(&json!({"status": "not_requested"})),
            "判断无需数据集供料"
        );
        assert_eq!(
            assistant_run_evidence_trail_label(&json!({"status": "empty"})),
            "尝试供料但无结果"
        );
        assert_eq!(
            assistant_run_evidence_trail_label(&json!({"status": "supplied"})),
            "检索供料证据"
        );
    }
}
