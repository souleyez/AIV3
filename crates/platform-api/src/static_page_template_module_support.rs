use serde_json::{json, Value};

use crate::static_page_template_adaptation_support::static_page_template_prompt_contains_any;
use crate::static_page_template_binding_support::static_page_template_request_needs_store_sales_binding;
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

pub(crate) fn static_page_template_adjusted_module_copy(
    reference: StaticPageTemplateReferenceSpec,
    module_id: &str,
    subject: &str,
    prompt: Option<&str>,
) -> Option<(String, String)> {
    let store_take_high = static_page_template_request_needs_store_sales_binding(prompt);
    let permission_views = static_page_template_prompt_contains_any(
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
    );
    match reference.id {
        "dashboard" => match module_id {
            "hero" => Some((
                format!("{subject}总览"),
                format!("按客户本轮意向概括{subject}当前状态、异常等级和最需要关注的判断。"),
            )),
            "kpi" => Some((
                if store_take_high {
                    "门店取高关键指标".to_string()
                } else {
                    "关键状态指标".to_string()
                },
                if store_take_high {
                    "展示门店/区域、销售额、租金、高分成和取高结果等核心指标；缺数时保留待补口径。"
                        .to_string()
                } else {
                    format!("围绕{subject}提炼 3-5 个能判断健康度、效率或风险的指标。")
                },
            )),
            "trend" => Some((
                if store_take_high {
                    "近7日经营趋势".to_string()
                } else {
                    "运行趋势".to_string()
                },
                format!("展示{subject}随时间、阶段或类别变化的方向，优先绑定可见数据。"),
            )),
            "risk" => Some((
                if permission_views {
                    "权限与风险预警".to_string()
                } else {
                    "风险预警".to_string()
                },
                if permission_views {
                    "区分总部和店总可见范围，标注越权风险、缺失字段和需要单独分发的链接。"
                        .to_string()
                } else {
                    format!("把{subject}的阻塞项、异常项和机会点按优先级展示。")
                },
            )),
            "activity" => Some((
                "后续动作".to_string(),
                format!("列出{subject}的调整、复核、发布和分发动作。"),
            )),
            _ => None,
        },
        "docs-page" => match module_id {
            "hero" => Some((
                format!("{subject}模板说明"),
                format!("说明{subject}模板适用对象、输出边界和当前资料完整度。"),
            )),
            "scope" => Some((
                "范围与边界".to_string(),
                format!("根据客户本轮意向列出{subject}的使用范围、权限边界和不可见内容。"),
            )),
            "steps" => Some((
                "使用流程".to_string(),
                format!("把{subject}模板的准备、填写、校验、生成和交付步骤拆清楚。"),
            )),
            "interfaces" => Some((
                "字段与数据".to_string(),
                format!("整理{subject}需要的字段、数据来源、样例行和缺失项。"),
            )),
            "checks" => Some((
                "验收与风险".to_string(),
                format!("列出{subject}模板交付后的验收标准、风险和待补信息。"),
            )),
            _ => None,
        },
        _ => match module_id {
            "hero" => Some((
                format!("{subject}核心结论"),
                format!("先给出{subject}最重要的业务判断，并说明来自当前授权数据和证据。"),
            )),
            "kpi" => Some((
                if store_take_high {
                    "门店取高核心 KPI".to_string()
                } else {
                    "关键指标与口径".to_string()
                },
                if store_take_high {
                    "围绕销售额、租金、高分成、取高结果和门店/区域筛选规划指标位；没有真实数值时显示待补口径。".to_string()
                } else {
                    format!("围绕{subject}规划 3-5 个关键指标位；没有真实数值时显示待补口径。")
                },
            )),
            "trend" => Some((
                if store_take_high {
                    "近7日/周期趋势".to_string()
                } else {
                    "趋势与变化".to_string()
                },
                format!("展示{subject}随时间、阶段或类别的变化方向，优先绑定当前可见字段。"),
            )),
            "comparison" => Some((
                if store_take_high {
                    "门店/区域对比".to_string()
                } else {
                    "分类对比".to_string()
                },
                format!(
                    "对{subject}的门店、区域、类别、角色或阶段差异做对比；缺少数据时保留补数提示。"
                ),
            )),
            "evidence" => Some((
                if permission_views {
                    "权限口径与证据缺口".to_string()
                } else {
                    "证据与缺口".to_string()
                },
                if permission_views {
                    "说明总部和分店店总各自可见字段、数据口径、证据来源和仍需补齐的信息。"
                        .to_string()
                } else {
                    format!("列出{subject}的证据来源、当前不可见内容和后续需要补齐的数据。")
                },
            )),
            _ => None,
        },
    }
}

fn static_page_template_module_id(module: &Value) -> &str {
    module
        .get("id")
        .or_else(|| module.get("role"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

pub(crate) fn static_page_template_module_intent_score(
    reference: StaticPageTemplateReferenceSpec,
    module: &Value,
    prompt: Option<&str>,
    original_index: usize,
) -> i64 {
    let module_id = static_page_template_module_id(module);
    if module_id == "hero" {
        return 10_000;
    }

    let mut score = 1_000_i64.saturating_sub(original_index as i64);
    let store_scope = static_page_template_prompt_contains_any(
        prompt,
        &["门店", "分店", "店铺", "区域", "store", "region", "area"],
    );
    let sales_take_high = static_page_template_prompt_contains_any(
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
    );
    let time_range = static_page_template_prompt_contains_any(
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
    );
    let permission_views = static_page_template_prompt_contains_any(
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
    );
    let template_or_field_request = static_page_template_prompt_contains_any(
        prompt,
        &[
            "模板",
            "字段",
            "表格",
            "要求文档",
            "格式",
            "template",
            "field",
        ],
    );

    if store_scope {
        score += match module_id {
            "kpi" => 700,
            "trend" => 560,
            "comparison" => 520,
            "risk" | "evidence" | "scope" => 180,
            "activity" | "steps" => 80,
            _ => 0,
        };
    }
    if sales_take_high {
        score += match module_id {
            "kpi" => 1_200,
            "trend" => 680,
            "comparison" => 620,
            "risk" | "evidence" => 160,
            _ => 0,
        };
    }
    if time_range {
        score += match module_id {
            "trend" => 920,
            "kpi" => 240,
            "comparison" => 180,
            "activity" | "steps" => 160,
            _ => 0,
        };
    }
    if permission_views {
        score += match module_id {
            "risk" | "evidence" | "scope" => 880,
            "comparison" | "interfaces" => 380,
            "kpi" => 140,
            "activity" | "steps" => 120,
            _ => 0,
        };
    }
    if template_or_field_request {
        score += match module_id {
            "interfaces" | "evidence" | "scope" => 360,
            "checks" | "risk" => 260,
            "comparison" => 120,
            _ => 0,
        };
    }

    if reference.id == "docs-page" {
        score += match module_id {
            "scope" => 80,
            "interfaces" => 70,
            "steps" => 50,
            "checks" => 40,
            _ => 0,
        };
    }

    score
}

fn static_page_template_layout_slot(
    reference: StaticPageTemplateReferenceSpec,
    index: usize,
) -> Value {
    let hero_height = if reference.id == "dashboard" { 2 } else { 3 };
    let (x, y, w, h) = match index {
        0 => (0, 0, 12, hero_height),
        1 => (0, hero_height, 5, 3),
        2 => (5, hero_height, 7, 3),
        3 => (0, hero_height + 3, 6, 4),
        4 => (6, hero_height + 3, 6, 4),
        _ => (0, hero_height + 7 + ((index as i64 - 5) * 4), 12, 4),
    };
    json!({
        "x": x,
        "y": y,
        "w": w,
        "h": h,
    })
}

pub(crate) fn static_page_template_apply_intent_ordered_layouts(
    modules: &mut [Value],
    reference: StaticPageTemplateReferenceSpec,
) {
    for (index, module) in modules.iter_mut().enumerate() {
        let Some(object) = module.as_object_mut() else {
            continue;
        };
        object.insert(
            "layout".to_string(),
            static_page_template_layout_slot(reference, index),
        );
        if let Some(adjustment) = object
            .get_mut("templateAdjustment")
            .and_then(Value::as_object_mut)
        {
            adjustment.insert(
                "moduleOrderPolicy".to_string(),
                json!("current_intent_highest_relevance_first"),
            );
            adjustment.insert("moduleOrderIndex".to_string(), json!(index));
        }
    }
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

    #[test]
    fn static_page_template_module_support_adjusts_take_high_metric_copy() {
        let copy = static_page_template_adjusted_module_copy(
            reference("data-report"),
            "kpi",
            "新百经营月报",
            Some("近7日销售按门店和区域切换，重点看租金取高"),
        )
        .expect("take-high kpi copy");

        assert_eq!(copy.0, "门店取高核心 KPI");
        assert!(copy.1.contains("销售额"));
        assert!(copy.1.contains("门店/区域筛选"));
    }

    #[test]
    fn static_page_template_module_support_adjusts_permission_copy() {
        let copy = static_page_template_adjusted_module_copy(
            reference("dashboard"),
            "risk",
            "门店经营",
            Some("总部和分店店总权限视角分别展示"),
        )
        .expect("permission risk copy");

        assert_eq!(copy.0, "权限与风险预警");
        assert!(copy.1.contains("总部"));
        assert!(copy.1.contains("店总"));
    }

    #[test]
    fn static_page_template_module_support_adjusts_docs_page_interfaces_copy() {
        let copy = static_page_template_adjusted_module_copy(
            reference("docs-page"),
            "interfaces",
            "数据库接入",
            Some("按模板字段输出"),
        )
        .expect("docs interfaces copy");

        assert_eq!(copy.0, "字段与数据");
        assert!(copy.1.contains("数据库接入"));
        assert!(copy.1.contains("样例行"));
    }

    #[test]
    fn static_page_template_module_support_returns_none_for_unknown_module() {
        assert!(static_page_template_adjusted_module_copy(
            reference("data-report"),
            "unknown-module",
            "经营报表",
            None,
        )
        .is_none());
    }

    #[test]
    fn static_page_template_module_support_module_id_prefers_id_and_falls_back_to_role() {
        assert_eq!(static_page_template_module_id(&json!({"id": "kpi"})), "kpi");
        assert_eq!(
            static_page_template_module_id(&json!({"role": "risk"})),
            "risk"
        );
        assert_eq!(static_page_template_module_id(&json!({"title": "空"})), "");
    }

    #[test]
    fn static_page_template_module_support_scores_hero_and_take_high_intent() {
        let reference = reference("data-report");
        let prompt = Some("销售取高按门店和区域筛选");

        let hero =
            static_page_template_module_intent_score(reference, &json!({"id": "hero"}), prompt, 4);
        let kpi =
            static_page_template_module_intent_score(reference, &json!({"id": "kpi"}), prompt, 0);
        let trend =
            static_page_template_module_intent_score(reference, &json!({"id": "trend"}), prompt, 0);

        assert_eq!(hero, 10_000);
        assert!(kpi > trend);
    }

    #[test]
    fn static_page_template_module_support_scores_docs_template_field_modules() {
        let reference = reference("docs-page");
        let prompt = Some("按模板字段和表格格式输出");

        let interfaces = static_page_template_module_intent_score(
            reference,
            &json!({"id": "interfaces"}),
            prompt,
            0,
        );
        let steps =
            static_page_template_module_intent_score(reference, &json!({"id": "steps"}), prompt, 0);

        assert!(interfaces > steps);
    }

    #[test]
    fn static_page_template_module_support_applies_layout_and_order_metadata() {
        let reference = reference("dashboard");
        let mut modules = vec![
            json!({"id": "hero", "templateAdjustment": {}}),
            json!({"id": "kpi", "templateAdjustment": {}}),
            json!("ignored"),
        ];

        static_page_template_apply_intent_ordered_layouts(&mut modules, reference);

        assert_eq!(
            modules[0]["layout"],
            json!({"x": 0, "y": 0, "w": 12, "h": 2})
        );
        assert_eq!(
            modules[1]["layout"],
            json!({"x": 0, "y": 2, "w": 5, "h": 3})
        );
        assert_eq!(
            modules[1]["templateAdjustment"]["moduleOrderPolicy"],
            "current_intent_highest_relevance_first"
        );
        assert_eq!(modules[1]["templateAdjustment"]["moduleOrderIndex"], 1);
        assert_eq!(modules[2], json!("ignored"));
    }
}
