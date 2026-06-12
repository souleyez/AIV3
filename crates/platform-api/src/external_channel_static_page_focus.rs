use serde_json::Value;

use crate::{static_page_prompt_focus_query_value, static_page_public_url_with_focus_label};

pub(crate) fn external_channel_static_page_customer_ready_text() -> &'static str {
    "已依据客户需求生成可访问的报表页面。"
}

fn external_channel_static_page_template_adaptation_from_payload(
    payload: &Value,
) -> Option<&Value> {
    [
        payload.get("template_adaptation"),
        payload.get("templateAdaptation"),
        payload.pointer("/source_refs/template_adaptation"),
        payload.pointer("/source_refs/templateAdaptation"),
    ]
    .into_iter()
    .flatten()
    .find(|value| !value.is_null())
}

fn external_channel_static_page_focus_label_from_url(public_url: &str) -> Option<String> {
    let url = reqwest::Url::parse(public_url).ok()?;
    url.query_pairs()
        .find(|(key, _)| key == "focus")
        .map(|(_, value)| value.into_owned())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn external_channel_static_page_user_intent_from_payload(
    payload: &Value,
) -> Option<&str> {
    [
        payload.pointer("/template_adaptation/userIntent"),
        payload.pointer("/templateAdaptation/userIntent"),
        payload.pointer("/template_adaptation/user_intent"),
        payload.pointer("/templateAdaptation/user_intent"),
        payload.pointer("/source_refs/template_adaptation/userIntent"),
        payload.pointer("/source_refs/templateAdaptation/userIntent"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(str::trim)
    .find(|value| !value.is_empty())
}

pub(crate) fn external_channel_static_page_focus_module_labels(
    payload: Option<&Value>,
) -> Vec<String> {
    let mut labels = Vec::new();
    let Some(payload) = payload else {
        return labels;
    };
    let Some(adaptation) = external_channel_static_page_template_adaptation_from_payload(payload)
    else {
        return labels;
    };
    let Some(focus_items) = adaptation.get("focus").and_then(Value::as_array) else {
        return labels;
    };
    for item in focus_items {
        let Some(label) = item.get("label").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if label.is_empty() || label == "当前意向优先" {
            continue;
        }
        if !labels.iter().any(|existing| existing == label) {
            labels.push(label.to_string());
        }
        if labels.len() >= 3 {
            break;
        }
    }
    labels
}

fn external_channel_static_page_default_modules_for_focus(focus: &str) -> Vec<String> {
    match focus {
        "取高机会" => vec![
            "取高线距离排行".to_string(),
            "当前/预测/取高线/需助推".to_string(),
            "预计取高增收".to_string(),
        ],
        "经营总览" => vec![
            "经营健康度".to_string(),
            "月度销售趋势".to_string(),
            "机会/风险品类占比".to_string(),
        ],
        "经营健康度" => vec![
            "经营健康度".to_string(),
            "月度销售趋势".to_string(),
            "机会/风险品类占比".to_string(),
        ],
        "风险店铺" => vec![
            "持续低活跃品牌".to_string(),
            "最新低活跃品牌".to_string(),
            "客流降低预警".to_string(),
        ],
        "低活跃" | "低活跃风险" => vec![
            "最新低活跃品牌".to_string(),
            "持续低活跃品牌".to_string(),
            "风险品类占比".to_string(),
        ],
        "品牌明细" => vec![
            "品牌/门店明细".to_string(),
            "合同与租金字段".to_string(),
            "可筛选明细表".to_string(),
        ],
        "品类业态" | "品类分析" => vec![
            "品类业态分布".to_string(),
            "区域/门店对比".to_string(),
            "结构变化分析".to_string(),
        ],
        _ => Vec::new(),
    }
}

fn external_channel_static_page_focus_label(
    payload: Option<&Value>,
    public_url: &str,
) -> Option<String> {
    if let Some(focus) = external_channel_static_page_focus_label_from_url(public_url) {
        return Some(focus);
    }
    if let Some(intent) = payload.and_then(external_channel_static_page_user_intent_from_payload) {
        if let Some(focus) = static_page_prompt_focus_query_value(intent) {
            return Some(focus.to_string());
        }
    }
    external_channel_static_page_focus_module_labels(payload)
        .into_iter()
        .next()
}

pub(crate) fn external_channel_static_page_public_url_with_payload_focus(
    public_url: &str,
    payload: &Value,
) -> String {
    if external_channel_static_page_focus_label_from_url(public_url).is_some() {
        return public_url.to_string();
    }
    let Some(focus) = external_channel_static_page_focus_label(Some(payload), public_url) else {
        return public_url.to_string();
    };
    static_page_public_url_with_focus_label(public_url, &focus)
}

pub(crate) fn external_channel_static_page_customer_ready_text_for_payload(
    base_text: &str,
    payload: Option<&Value>,
    public_url: &str,
) -> String {
    let mut text = base_text.to_string();
    let focus = external_channel_static_page_focus_label(payload, public_url);
    if let Some(focus) = focus.as_deref() {
        text.push_str("\n系统识别到本轮关注焦点：");
        text.push_str(focus);
        text.push('。');
    }
    let mut modules = external_channel_static_page_focus_module_labels(payload);
    if modules.is_empty() {
        if let Some(focus) = focus.as_deref() {
            modules = external_channel_static_page_default_modules_for_focus(focus);
        }
    }
    if !modules.is_empty() {
        text.push_str("\n报表会优先呈现：");
        text.push_str(&modules.join("、"));
        text.push('。');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn artifact_url() -> &'static str {
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/index.html"
    }

    #[test]
    fn public_url_focus_takes_precedence_over_payload_intent() {
        let payload = json!({
            "template_adaptation": {
                "userIntent": "生成低活跃品牌报表"
            }
        });

        let focused = external_channel_static_page_public_url_with_payload_focus(
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/index.html?focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A",
            &payload,
        );

        assert!(focused.contains("focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A"));
        assert!(!focused.contains("%E4%BD%8E%E6%B4%BB%E8%B7%83%E9%A3%8E%E9%99%A9"));
    }

    #[test]
    fn focus_module_labels_skip_generic_labels_dedupe_and_limit() {
        let payload = json!({
            "templateAdaptation": {
                "focus": [
                    {"label": "当前意向优先"},
                    {"label": " 取高线距离排行 "},
                    {"label": "取高线距离排行"},
                    {"label": "当前/预测/取高线/需助推"},
                    {"label": "预计取高增收"},
                    {"label": "第四项不应出现"}
                ]
            }
        });

        assert_eq!(
            external_channel_static_page_focus_module_labels(Some(&payload)),
            vec![
                "取高线距离排行".to_string(),
                "当前/预测/取高线/需助推".to_string(),
                "预计取高增收".to_string(),
            ]
        );
    }

    #[test]
    fn customer_ready_text_uses_default_modules_for_prompt_focus() {
        let text = external_channel_static_page_customer_ready_text_for_payload(
            external_channel_static_page_customer_ready_text(),
            Some(&json!({
                "template_adaptation": {
                    "userIntent": "看经营状况"
                }
            })),
            artifact_url(),
        );

        assert!(text.contains("系统识别到本轮关注焦点：经营总览"));
        assert!(text.contains("报表会优先呈现：经营健康度、月度销售趋势、机会/风险品类占比"));
    }
}
