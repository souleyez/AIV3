use serde_json::{json, Value};

use crate::static_page_template_reference_support::StaticPageTemplateReferenceSpec;

fn static_page_template_data_binding(source_id: &str) -> Value {
    match source_id {
        "session" => json!({
            "type": "conversation_summary",
            "label": "来自当前会话摘要",
            "sourceId": "session",
        }),
        "dataset" => json!({
            "type": "dataset_metrics",
            "label": "来自数据集指标摘要",
            "sourceId": "dataset",
        }),
        "evidence" => json!({
            "type": "retrieval_evidence",
            "label": "来自检索证据",
            "sourceId": "evidence",
        }),
        "selected_scope" => json!({
            "type": "selected_scope",
            "label": "来自当前选中范围",
            "sourceId": "selected_scope",
        }),
        _ => json!({
            "type": "model_summary",
            "label": "来自模型总结",
            "sourceId": "model",
        }),
    }
}

fn static_page_template_module(
    id: &str,
    role: &str,
    title: &str,
    content: &str,
    data_source: &str,
    visualization_type: &str,
    visualization_label: &str,
    layout: (i64, i64, i64, i64),
) -> Value {
    json!({
        "id": id,
        "role": role,
        "title": title,
        "content": content,
        "dataBinding": static_page_template_data_binding(data_source),
        "visualization": {
            "type": visualization_type,
            "label": visualization_label,
        },
        "layout": {
            "x": layout.0,
            "y": layout.1,
            "w": layout.2,
            "h": layout.3,
        },
    })
}

pub(crate) fn static_page_template_modules(reference: StaticPageTemplateReferenceSpec) -> Value {
    let modules = match reference.id {
        "dashboard" => vec![
            static_page_template_module(
                "hero",
                "hero",
                "运营总览",
                "总结当前状态、异常等级和本轮关注重点。",
                "session",
                "headline",
                "大标题 + 关键结论",
                (0, 0, 12, 2),
            ),
            static_page_template_module(
                "kpi",
                "metrics",
                "关键状态",
                "显示 3-5 个用于判断健康度、效率、进度或风险的指标。",
                "dataset",
                "kpi-cards",
                "关键指标卡",
                (0, 2, 5, 3),
            ),
            static_page_template_module(
                "trend",
                "trend",
                "运行趋势",
                "展示任务量、转化、响应、质量或异常的时间走势。",
                "evidence",
                "line-chart",
                "趋势折线图",
                (5, 2, 7, 3),
            ),
            static_page_template_module(
                "risk",
                "risk",
                "风险预警",
                "把阻塞项、异常项和机会点按优先级展示。",
                "evidence",
                "risk-matrix",
                "风险优先级矩阵",
                (0, 5, 6, 4),
            ),
            static_page_template_module(
                "activity",
                "activity",
                "最近动作",
                "列出最近更新、待处理动作和负责人线索；不可见时标注未供料。",
                "model",
                "timeline",
                "阶段时间线",
                (6, 5, 6, 4),
            ),
        ],
        "docs-page" => vec![
            static_page_template_module(
                "hero",
                "hero",
                "文档概览",
                "说明这份文档解决什么问题、适用对象和当前信息完整度。",
                "session",
                "headline",
                "大标题 + 关键结论",
                (0, 0, 12, 3),
            ),
            static_page_template_module(
                "scope",
                "scope",
                "范围与边界",
                "列出系统边界、权限边界、已供料和未供料范围。",
                "evidence",
                "text-insight",
                "洞察文本块",
                (0, 3, 6, 3),
            ),
            static_page_template_module(
                "steps",
                "steps",
                "流程步骤",
                "把关键流程拆成可执行步骤，保留前后依赖。",
                "model",
                "timeline",
                "阶段时间线",
                (6, 3, 6, 3),
            ),
            static_page_template_module(
                "interfaces",
                "interfaces",
                "接口与数据",
                "整理接口、字段、输入输出或配置项；没有真实接口时标注待补。",
                "selected_scope",
                "text-insight",
                "洞察文本块",
                (0, 6, 6, 4),
            ),
            static_page_template_module(
                "checks",
                "checks",
                "校验与交付",
                "列出验证命令、验收标准、风险和下一步交付动作。",
                "model",
                "text-insight",
                "洞察文本块",
                (6, 6, 6, 4),
            ),
        ],
        _ => vec![
            static_page_template_module(
                "hero",
                "hero",
                "报告结论",
                "用一句话说明当前数据最重要的业务判断，并标出数据来源状态。",
                "session",
                "headline",
                "大标题 + 关键结论",
                (0, 0, 12, 3),
            ),
            static_page_template_module(
                "kpi",
                "metrics",
                "核心 KPI",
                "提炼 3-5 个最重要指标；没有可见数值时明确标注需要补充数据。",
                "dataset",
                "kpi-cards",
                "关键指标卡",
                (0, 3, 5, 3),
            ),
            static_page_template_module(
                "trend",
                "trend",
                "趋势变化",
                "展示关键指标随时间、阶段或类别的变化方向。",
                "evidence",
                "line-chart",
                "趋势折线图",
                (5, 3, 7, 3),
            ),
            static_page_template_module(
                "comparison",
                "comparison",
                "分类对比",
                "用对比图解释不同渠道、品类、地区或阶段的差异。",
                "selected_scope",
                "bar-chart",
                "分类对比柱状图",
                (0, 6, 6, 4),
            ),
            static_page_template_module(
                "evidence",
                "evidence",
                "证据与方法",
                "列出数据口径、可见证据和当前缺失项，避免模板隐藏不完整数据。",
                "evidence",
                "text-insight",
                "洞察文本块",
                (6, 6, 6, 4),
            ),
        ],
    };
    Value::Array(modules)
}

pub(crate) fn static_page_template_mobile_order(modules: &Value) -> Value {
    Value::Array(
        modules
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|module| module.get("id").and_then(Value::as_str))
            .map(|id| json!(id))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(id: &'static str) -> StaticPageTemplateReferenceSpec {
        StaticPageTemplateReferenceSpec {
            id,
            label: "测试模板",
            category: "test",
            scenario: "test",
            style_direction: "test",
            aspect_hint: "test",
            design_intent: "test",
            objective: "test",
            audience: "test",
            prompt_hints: &[],
        }
    }

    fn module_ids(modules: &Value) -> Vec<&str> {
        modules
            .as_array()
            .expect("modules array")
            .iter()
            .map(|module| module["id"].as_str().expect("module id"))
            .collect()
    }

    #[test]
    fn static_page_template_module_support_data_report_seed_keeps_expected_order() {
        let modules = static_page_template_modules(reference("data-report"));

        assert_eq!(
            module_ids(&modules),
            vec!["hero", "kpi", "trend", "comparison", "evidence"]
        );
        assert_eq!(modules[1]["dataBinding"]["sourceId"], "dataset");
        assert_eq!(modules[3]["visualization"]["type"], "bar-chart");
        assert_eq!(
            static_page_template_mobile_order(&modules),
            json!(["hero", "kpi", "trend", "comparison", "evidence"])
        );
    }

    #[test]
    fn static_page_template_module_support_dashboard_seed_keeps_risk_and_activity_roles() {
        let modules = static_page_template_modules(reference("dashboard"));

        assert_eq!(
            module_ids(&modules),
            vec!["hero", "kpi", "trend", "risk", "activity"]
        );
        assert_eq!(modules[3]["visualization"]["type"], "risk-matrix");
        assert_eq!(modules[4]["dataBinding"]["sourceId"], "model");
    }

    #[test]
    fn static_page_template_module_support_docs_page_seed_keeps_interface_scope_binding() {
        let modules = static_page_template_modules(reference("docs-page"));

        assert_eq!(
            module_ids(&modules),
            vec!["hero", "scope", "steps", "interfaces", "checks"]
        );
        assert_eq!(modules[3]["role"], "interfaces");
        assert_eq!(modules[3]["dataBinding"]["sourceId"], "selected_scope");
    }

    #[test]
    fn static_page_template_module_support_unknown_reference_uses_data_report_shape() {
        let modules = static_page_template_modules(reference("unknown"));

        assert_eq!(
            module_ids(&modules),
            vec!["hero", "kpi", "trend", "comparison", "evidence"]
        );
        assert_eq!(modules[4]["role"], "evidence");
    }
}
