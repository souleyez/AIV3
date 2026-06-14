use serde_json::{json, Value};

use crate::static_page_prompt_requests_explicit_redesign;
use crate::static_page_template_binding_support::static_page_template_text_contains_any;

pub(crate) fn static_page_template_prompt_from_payload(payload: &Value) -> Option<&str> {
    payload
        .get("prompt")
        .or_else(|| payload.get("promptText"))
        .or_else(|| payload.get("prompt_text"))
        .or_else(|| payload.get("templateIntent"))
        .or_else(|| payload.get("template_intent"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn static_page_template_prompt_subject(
    prompt: Option<&str>,
    reference_label: &str,
) -> String {
    let Some(prompt) = prompt else {
        return reference_label.to_string();
    };
    let compact = prompt.split_whitespace().collect::<Vec<_>>().join("");
    let mut subject = compact
        .replace("帮我", "")
        .replace("请", "")
        .replace("能不能", "")
        .replace("能否", "")
        .replace("可以", "")
        .replace("提供", "")
        .replace("一个", "")
        .replace("一份", "")
        .replace("模板", "")
        .replace("报表", "")
        .replace("静态页", "")
        .replace("页面", "")
        .replace("看看", "")
        .replace("生成", "")
        .replace("制作", "")
        .replace("输出", "")
        .replace("做", "");
    subject = subject
        .trim_matches(|ch: char| {
            ch.is_ascii_punctuation()
                || matches!(ch, '，' | '。' | '；' | '：' | '、' | '！' | '？')
        })
        .chars()
        .take(28)
        .collect::<String>();
    if subject.chars().count() >= 2 {
        subject
    } else {
        reference_label.to_string()
    }
}

pub(crate) fn static_page_template_prompt_contains_any(
    prompt: Option<&str>,
    needles: &[&str],
) -> bool {
    let Some(prompt) = prompt else {
        return false;
    };
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let lower = compact.to_ascii_lowercase();
    static_page_template_text_contains_any(&compact, &lower, needles)
}

pub(crate) fn static_page_template_adaptation_focus(prompt: Option<&str>) -> Vec<Value> {
    let mut focus = Vec::new();
    if static_page_template_prompt_contains_any(
        prompt,
        &["门店", "分店", "店铺", "区域", "store", "region", "area"],
    ) {
        focus.push(json!({
            "code": "store_or_region_scope",
            "label": "门店/区域维度",
            "instruction": "模板模块需要支持按门店、分店或区域筛选和对比。"
        }));
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "取高",
            "高分成",
            "销售",
            "营业额",
            "营收",
            "租金",
            "sales",
            "revenue",
            "rent",
        ],
    ) {
        focus.push(json!({
            "code": "sales_take_high_metric",
            "label": "销售/租金/取高口径",
            "instruction": "模板指标位需要围绕销售额、租金、高分成或取高逻辑重排。"
        }));
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "低活跃",
            "不活跃",
            "零销售",
            "无销售",
            "连续无销售",
            "低销售",
            "风险品牌",
            "风险门店",
            "客流下降",
            "客流降低",
            "客流预警",
            "inactive",
        ],
    ) {
        focus.push(json!({
            "code": "low_activity_risk_modules",
            "label": "低活跃风险模块",
            "instruction": "模板需要前置风险品类占比、最新低活跃品牌、持续低活跃品牌和客流降低预警。"
        }));
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "风险识别",
            "风险识别系统",
            "风险店铺",
            "风险门店",
            "风险品牌",
            "风险提示",
            "高风险",
            "风险预警",
            "risk",
        ],
    ) {
        focus.push(json!({
            "code": "low_activity_risk_modules",
            "label": "低活跃风险模块",
            "instruction": "模板需要优先突出风险相关店铺清单与风险占比指标。"
        }));
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "近7日",
            "近七日",
            "月",
            "季度",
            "年度",
            "time",
            "date",
            "month",
        ],
    ) {
        focus.push(json!({
            "code": "time_range_controls",
            "label": "时间筛选",
            "instruction": "模板需要保留时间范围筛选、趋势图和动态刷新口径。"
        }));
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "权限",
            "角色",
            "总部",
            "店总",
            "分店店总",
            "recipient",
            "role",
            "permission",
        ],
    ) {
        focus.push(json!({
            "code": "role_permission_views",
            "label": "角色权限视角",
            "instruction": "模板需要区分总部视角和分店店总视角，说明哪些字段可见、哪些需要隐藏或分发不同链接。"
        }));
    }
    if focus.is_empty() {
        focus.push(json!({
            "code": "current_intent_first",
            "label": "当前意向优先",
            "instruction": "模板先按客户本轮目标调整标题、模块顺序和指标占位，再提供给客户。"
        }));
    }
    focus
}

fn static_page_template_patch_contract(
    reference_id: Option<&str>,
    prompt: Option<&str>,
    focus: &[Value],
) -> Value {
    let explicit_redesign =
        prompt.is_some_and(|prompt| static_page_prompt_requests_explicit_redesign(prompt));
    let time_range = static_page_template_patch_time_range(prompt);
    let chart_requests = static_page_template_patch_chart_requests(prompt);
    let intent_classes = static_page_template_patch_intent_classes(
        prompt,
        focus,
        time_range.as_deref(),
        &chart_requests,
        explicit_redesign,
    );
    let focus_code = static_page_template_patch_focus_code(prompt, focus);
    let operation = if explicit_redesign {
        "generate_new_template"
    } else {
        "patch_existing_template"
    };
    let preserve_style = !explicit_redesign;
    json!({
        "operation": operation,
        "template_id": reference_id,
        "templateId": reference_id,
        "preserve_style": preserve_style,
        "preserveStyle": preserve_style,
        "requires_new_image2": explicit_redesign,
        "requiresNewImage2": explicit_redesign,
        "intent_classes": intent_classes,
        "intentClasses": intent_classes,
        "focus": focus_code,
        "time_range": time_range,
        "timeRange": time_range,
        "chart_requests": chart_requests,
        "chartRequests": chart_requests,
        "data_refresh": true,
        "dataRefresh": true,
        "allowed_outputs": [
            "data.json",
            "data-snapshot.json",
            "focus_query",
            "module_order",
            "chart_data",
            "csv",
            "ppt",
            "markdown"
        ],
        "forbidden_outputs": [
            "generic_html_final_fallback",
            "random_template_switch",
            "temporary_page_as_default_template",
            "customer_visible_smoke_or_prewarm"
        ]
    })
}

fn static_page_template_patch_intent_classes(
    prompt: Option<&str>,
    focus: &[Value],
    time_range: Option<&str>,
    chart_requests: &[Value],
    explicit_redesign: bool,
) -> Vec<String> {
    let mut classes = Vec::new();
    if explicit_redesign {
        classes.push("explicit_redesign".to_string());
    }
    if time_range.is_some() {
        classes.push("time_range_change".to_string());
    }
    if static_page_template_patch_focus_code(prompt, focus).is_some() {
        classes.push("focus_change".to_string());
    }
    if !chart_requests.is_empty() {
        classes.push("chart_change".to_string());
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "模块顺序",
            "排序",
            "前置",
            "置顶",
            "放到",
            "提到",
            "module order",
            "reorder",
        ],
    ) {
        classes.push("module_reorder".to_string());
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "新增指标",
            "增加指标",
            "加指标",
            "新增字段",
            "增加字段",
            "metric",
            "add metric",
        ],
    ) {
        classes.push("metric_addition".to_string());
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "刷新数据",
            "更新数据",
            "文档",
            "文件",
            "上传",
            "excel",
            "csv",
            "合同",
            "说明",
            "document",
            "refresh data",
        ],
    ) {
        classes.push("document_data_refresh".to_string());
    }
    if classes.is_empty() {
        classes.push("current_intent_patch".to_string());
    }
    classes
}

fn static_page_template_patch_focus_code(prompt: Option<&str>, focus: &[Value]) -> Option<String> {
    if static_page_template_prompt_contains_any(prompt, &["取高", "高分成", "缺口"]) {
        return Some("take_high_opportunity".to_string());
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "低活跃",
            "不活跃",
            "风险",
            "零销售",
            "无销售",
            "客流下降",
            "inactive",
            "risk",
        ],
    ) {
        return Some("low_activity_risk".to_string());
    }
    if static_page_template_prompt_contains_any(
        prompt,
        &["门店", "分店", "店铺", "区域", "store", "region", "area"],
    ) {
        return Some("store_or_region_scope".to_string());
    }
    focus
        .iter()
        .filter_map(|item| item.get("code").and_then(Value::as_str))
        .find(|code| *code != "current_intent_first")
        .map(ToOwned::to_owned)
}

fn static_page_template_patch_time_range(prompt: Option<&str>) -> Option<String> {
    if static_page_template_prompt_contains_any(
        prompt,
        &[
            "最近一个月",
            "近一个月",
            "最近1个月",
            "近1个月",
            "本月",
            "月报",
            "latest month",
        ],
    ) {
        return Some("latest_month".to_string());
    }
    if static_page_template_prompt_contains_any(prompt, &["近7日", "近七日", "最近7天", "last 7"])
    {
        return Some("last_7_days".to_string());
    }
    if static_page_template_prompt_contains_any(prompt, &["季度", "quarter"]) {
        return Some("latest_quarter".to_string());
    }
    if static_page_template_prompt_contains_any(prompt, &["年度", "全年", "year"]) {
        return Some("latest_year".to_string());
    }
    None
}

fn static_page_template_patch_chart_requests(prompt: Option<&str>) -> Vec<Value> {
    let Some(chart_type) = static_page_template_patch_chart_type(prompt) else {
        return Vec::new();
    };
    vec![json!({
        "module": "comparison",
        "type": chart_type,
        "metric": static_page_template_patch_metric(prompt),
        "dimension": static_page_template_patch_dimension(prompt),
    })]
}

fn static_page_template_patch_chart_type(prompt: Option<&str>) -> Option<&'static str> {
    if static_page_template_prompt_contains_any(prompt, &["柱状图", "柱形图", "bar chart", "bar"])
    {
        return Some("bar");
    }
    if static_page_template_prompt_contains_any(prompt, &["折线图", "趋势图", "line chart", "line"])
    {
        return Some("line");
    }
    if static_page_template_prompt_contains_any(prompt, &["饼图", "占比图", "pie chart", "pie"])
    {
        return Some("pie");
    }
    None
}

fn static_page_template_patch_metric(prompt: Option<&str>) -> &'static str {
    if static_page_template_prompt_contains_any(prompt, &["月租金", "租金", "yuezujin", "rent"])
    {
        return "yuezujin";
    }
    if static_page_template_prompt_contains_any(prompt, &["取高", "高分成", "缺口"]) {
        return "take_high_gap";
    }
    if static_page_template_prompt_contains_any(prompt, &["销售", "营业额", "营收", "sales"])
    {
        return "sales_amount";
    }
    "auto"
}

fn static_page_template_patch_dimension(prompt: Option<&str>) -> &'static str {
    if static_page_template_prompt_contains_any(prompt, &["区域", "分区", "dist", "region"]) {
        return "dist_name";
    }
    if static_page_template_prompt_contains_any(prompt, &["门店", "分店", "店铺", "store"]) {
        return "shopdesc";
    }
    if static_page_template_prompt_contains_any(prompt, &["品牌", "brand"]) {
        return "brandcode";
    }
    "auto"
}

pub(crate) fn static_page_template_adaptation_plan(
    reference_label: &str,
    reference_id: Option<&str>,
    prompt: Option<&str>,
    evidence_summary: Option<&Value>,
    missing_evidence: Option<&Value>,
) -> Value {
    let subject = static_page_template_prompt_subject(prompt, reference_label);
    let focus = static_page_template_adaptation_focus(prompt);
    let patch_contract = static_page_template_patch_contract(reference_id, prompt, &focus);
    json!({
        "policy": "adapt_template_before_delivery",
        "templateUse": "style_structure_adjusted_to_current_intent",
        "templateReferenceId": reference_id,
        "templateLabel": reference_label,
        "userIntent": prompt,
        "adaptedSubject": subject,
        "summary": format!("已基于客户本轮意向调整「{reference_label}」模板后再提供；模板只控制结构、版式和字段组织，事实内容仍以当前授权数据和证据为准。"),
        "focus": focus,
        "patchContract": patch_contract.clone(),
        "patch_contract": patch_contract,
        "moduleOrderRule": "current_intent_highest_relevance_first",
        "deliveryRule": "do_not_return_raw_or_generic_template; return adjusted_template_draft_or_continue_artifact_generation",
        "evidenceSummary": evidence_summary.cloned().unwrap_or(Value::Null),
        "missingEvidence": missing_evidence.cloned().unwrap_or(Value::Null),
    })
}

fn static_page_template_reference_value_label(reference: &Value) -> &str {
    reference
        .get("label")
        .or_else(|| reference.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("模板参考")
}

fn static_page_template_reference_value_id(reference: &Value) -> Option<&str> {
    reference
        .get("templateId")
        .or_else(|| reference.get("template_id"))
        .or_else(|| reference.get("id"))
        .and_then(Value::as_str)
}

pub(crate) fn static_page_template_adaptation_plan_from_reference_value(
    reference: &Value,
    prompt: Option<&str>,
    evidence_summary: Option<&Value>,
    missing_evidence: Option<&Value>,
) -> Value {
    static_page_template_adaptation_plan(
        static_page_template_reference_value_label(reference),
        static_page_template_reference_value_id(reference),
        prompt,
        evidence_summary,
        missing_evidence,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_array(value: Value) -> Vec<Value> {
        value.as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn prompt_subject_strips_request_words_and_falls_back() {
        assert_eq!(
            static_page_template_prompt_subject(Some("请帮我生成低活跃品牌报表模板"), "数据报告"),
            "低活跃品牌"
        );
        assert_eq!(
            static_page_template_prompt_subject(Some("做"), "数据报告"),
            "数据报告"
        );
        assert_eq!(
            static_page_template_prompt_subject(None, "数据报告"),
            "数据报告"
        );
    }

    #[test]
    fn adaptation_focus_extracts_risk_time_and_permission_intent() {
        let focus =
            static_page_template_adaptation_focus(Some("近7日低活跃风险门店，区分总部和店总权限"));

        assert!(focus
            .iter()
            .any(|item| item["code"] == json!("low_activity_risk_modules")));
        assert!(focus
            .iter()
            .any(|item| item["code"] == json!("time_range_controls")));
        assert!(focus
            .iter()
            .any(|item| item["code"] == json!("role_permission_views")));
    }

    #[test]
    fn adaptation_plan_keeps_patch_contract_for_non_redesign_updates() {
        let plan = static_page_template_adaptation_plan(
            "新百经营分析模板",
            Some("xinbai-functional-modular-template-20260604"),
            Some("给一个按区域分析月租金的柱状图"),
            Some(&json!({"status": "supplied"})),
            Some(&json!({"status": "ready"})),
        );

        assert_eq!(
            plan["patch_contract"]["operation"],
            json!("patch_existing_template")
        );
        assert_eq!(plan["patch_contract"]["requires_new_image2"], json!(false));
        assert_eq!(plan["patch_contract"]["time_range"], Value::Null);
        assert_eq!(
            plan["patch_contract"]["chart_requests"][0]["type"],
            json!("bar")
        );
        assert_eq!(
            plan["patch_contract"]["chart_requests"][0]["metric"],
            json!("yuezujin")
        );
        assert_eq!(
            plan["patch_contract"]["chart_requests"][0]["dimension"],
            json!("dist_name")
        );
        assert!(
            value_array(plan["patch_contract"]["intent_classes"].clone())
                .iter()
                .any(|class| class == "chart_change")
        );
        assert_eq!(plan["evidenceSummary"]["status"], json!("supplied"));
        assert_eq!(plan["missingEvidence"]["status"], json!("ready"));
    }

    #[test]
    fn only_explicit_redesign_requires_new_image2() {
        let plan = static_page_template_adaptation_plan(
            "新百经营分析模板",
            Some("xinbai-functional-modular-template-20260604"),
            Some("重新设计暗色移动端版本"),
            None,
            None,
        );

        assert_eq!(
            plan["patch_contract"]["operation"],
            json!("generate_new_template")
        );
        assert_eq!(plan["patch_contract"]["requires_new_image2"], json!(true));
        assert_eq!(plan["patch_contract"]["preserve_style"], json!(false));
        assert!(
            value_array(plan["patch_contract"]["intent_classes"].clone())
                .iter()
                .any(|class| class == "explicit_redesign")
        );
    }
}
