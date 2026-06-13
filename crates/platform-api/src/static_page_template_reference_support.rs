use serde_json::{json, Value};

use crate::ApiError;

#[derive(Clone, Copy, Debug)]
pub(crate) struct StaticPageTemplateReferenceSpec {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) category: &'static str,
    pub(crate) scenario: &'static str,
    pub(crate) style_direction: &'static str,
    pub(crate) aspect_hint: &'static str,
    pub(crate) design_intent: &'static str,
    pub(crate) objective: &'static str,
    pub(crate) audience: &'static str,
    pub(crate) prompt_hints: &'static [&'static str],
}

pub(crate) const STATIC_PAGE_TEMPLATE_REFERENCE_GUARDRAILS: &[&str] = &[
    "template reference controls style and module recipe only",
    "DataMax model routing, permissions, datasets, evidence, and artifacts remain authoritative",
    "model output must become structured draft data, not raw final HTML",
    "missing or partial evidence must stay visible in draft and rendered output",
];

pub(crate) const STATIC_PAGE_TEMPLATE_FORBIDDEN_OUTPUTS: &[&str] = &[
    "raw_html",
    "remote_script",
    "remote_css",
    "provider_secret",
    "queue_credential",
    "private_path",
];

pub(crate) fn normalize_static_page_template_reference_id(raw_id: Option<&str>) -> Option<&str> {
    raw_id.map(str::trim).filter(|value| !value.is_empty())
}

fn static_page_template_reference_id_from_array(value: &Value) -> Option<&str> {
    value.as_array().and_then(|items| {
        items.iter().find_map(|item| {
            normalize_static_page_template_reference_id(
                item.get("templateId")
                    .or_else(|| item.get("template_id"))
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str),
            )
        })
    })
}

pub(crate) fn static_page_template_reference_id_from_payload(payload: &Value) -> Option<&str> {
    normalize_static_page_template_reference_id(
        payload
            .get("templateReferenceId")
            .or_else(|| payload.get("template_reference_id"))
            .and_then(Value::as_str),
    )
    .or_else(|| {
        payload
            .get("designReferences")
            .or_else(|| payload.get("design_references"))
            .and_then(static_page_template_reference_id_from_array)
    })
    .or_else(|| {
        payload
            .get("source")
            .and_then(|source| {
                source
                    .get("templateReferences")
                    .or_else(|| source.get("template_references"))
            })
            .and_then(static_page_template_reference_id_from_array)
    })
}

pub(crate) fn static_page_template_reference_id_from_source_refs(
    source_refs: &Value,
) -> Option<&str> {
    normalize_static_page_template_reference_id(
        source_refs
            .get("template_reference_id")
            .or_else(|| source_refs.get("templateReferenceId"))
            .and_then(Value::as_str),
    )
    .or_else(|| {
        source_refs
            .get("template_references")
            .or_else(|| source_refs.get("templateReferences"))
            .and_then(static_page_template_reference_id_from_array)
    })
}

fn static_page_template_intent_contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| text.contains(keyword))
}

pub(crate) fn infer_static_page_template_reference_id(intent: &str) -> Option<&'static str> {
    let text = intent.trim().to_ascii_lowercase();
    if text.is_empty() {
        return None;
    }
    if static_page_template_intent_contains_any(
        &text,
        &[
            "文档",
            "技术方案",
            "接口",
            "api",
            "readme",
            "说明书",
            "交接",
            "handoff",
            "sop",
            "教程",
            "操作手册",
            "验收",
        ],
    ) {
        return Some("docs-page");
    }
    if static_page_template_intent_contains_any(
        &text,
        &[
            "看板",
            "仪表板",
            "仪表盘",
            "dashboard",
            "后台",
            "运营总览",
            "监控",
            "状态总览",
            "实时状态",
            "overview",
        ],
    ) {
        return Some("dashboard");
    }
    if static_page_template_intent_contains_any(
        &text,
        &[
            "报告",
            "分析",
            "经营",
            "数据可视化",
            "图表",
            "指标",
            "kpi",
            "report",
            "analysis",
            "metrics",
            "one-pager",
            "one pager",
        ],
    ) {
        return Some("data-report");
    }
    None
}

pub(crate) fn resolve_static_page_template_reference(
    raw_id: Option<&str>,
) -> std::result::Result<Option<StaticPageTemplateReferenceSpec>, ApiError> {
    let Some(raw_id) = raw_id else {
        return Ok(None);
    };
    let id = raw_id.trim().to_ascii_lowercase();
    if id.is_empty() {
        return Ok(None);
    }
    if id.starts_with("document-template-") {
        return Ok(None);
    }

    match id.as_str() {
        "data-report" => Ok(Some(StaticPageTemplateReferenceSpec {
            id: "data-report",
            label: "数据可视化报告",
            category: "data",
            scenario: "finance",
            style_direction: "data-command",
            aspect_hint: "desktop-long-page",
            design_intent:
                "把可见 CSV、Excel、JSON、文档指标或会话数据整理成 KPI、趋势、对比和证据表。",
            objective: "快速生成一页数据可视化报告，展示关键指标、趋势、结构和可核查证据。",
            audience: "业务负责人和客户决策层",
            prompt_hints: &[
                "prefer KPI cards, trend charts, comparison charts, and evidence notes",
                "never invent numbers; ask DataMax retrieval or data repair when chart rows are missing",
            ],
        })),
        "dashboard" => Ok(Some(StaticPageTemplateReferenceSpec {
            id: "dashboard",
            label: "管理后台仪表板",
            category: "dashboard",
            scenario: "operations",
            style_direction: "data-command",
            aspect_hint: "desktop-dashboard",
            design_intent: "把运营状态整理成密集但可扫描的 KPI、趋势、风险和最近活动。",
            objective: "快速生成一页运营仪表板，帮助用户扫清当前状态、异常和下一步动作。",
            audience: "运营负责人和项目管理人员",
            prompt_hints: &[
                "favor dense status scanning over marketing hero composition",
                "surface unresolved risks and missing operational evidence",
            ],
        })),
        "docs-page" => Ok(Some(StaticPageTemplateReferenceSpec {
            id: "docs-page",
            label: "技术文档页",
            category: "doc",
            scenario: "engineering",
            style_direction: "client-delivery",
            aspect_hint: "documentation-page",
            design_intent: "把文档、接口说明或方案内容整理成清晰的阅读页。",
            objective: "快速生成一页技术文档或交接说明，保留结构、步骤、注意事项和缺失信息。",
            audience: "技术对接人员和项目成员",
            prompt_hints: &[
                "preserve source headings and section hierarchy when DataMax supplied document detail",
                "show unavailable interface details as missing evidence instead of guessing",
            ],
        })),
        "deck-swiss-international" | "video-hyperframes" => Err(ApiError::bad_request(
            "static_page_template_reference_paused",
            format!("{id} belongs to a paused PPT/video track and cannot create DataMax static pages"),
        )),
        _ => Err(ApiError::bad_request(
            "invalid_static_page_template_reference",
            format!("{id} is not an enabled DataMax static page template reference"),
        )),
    }
}

pub(crate) fn static_page_template_design_reference(
    reference: StaticPageTemplateReferenceSpec,
) -> Value {
    json!({
        "source": "html-anything",
        "sourceKind": "template_design_reference",
        "upstream": "nexu-io/html-anything",
        "license": "Apache-2.0",
        "importPolicy": "metadata_and_constraints_only",
        "templateId": reference.id,
        "label": reference.label,
        "category": reference.category,
        "scenario": reference.scenario,
        "surface": "static_page",
        "status": "enabled",
        "pauseReason": "",
        "quickOutput": true,
        "aspectHint": reference.aspect_hint,
        "styleDirection": reference.style_direction,
        "designIntent": reference.design_intent,
        "promptHints": reference.prompt_hints,
        "guardrails": STATIC_PAGE_TEMPLATE_REFERENCE_GUARDRAILS,
        "providerPolicy": {
            "providerOutput": "structured_static_page_draft_json",
            "forbiddenOutput": STATIC_PAGE_TEMPLATE_FORBIDDEN_OUTPUTS,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_reference_extracts_payload_and_source_ref_ids() {
        let payload = json!({
            "source": {
                "templateReferences": [{ "templateId": "docs-page" }]
            }
        });
        assert_eq!(
            static_page_template_reference_id_from_payload(&payload),
            Some("docs-page")
        );

        let source_refs = json!({
            "template_references": [{ "template_id": "dashboard" }]
        });
        assert_eq!(
            static_page_template_reference_id_from_source_refs(&source_refs),
            Some("dashboard")
        );
        assert_eq!(
            normalize_static_page_template_reference_id(Some(" data-report ")),
            Some("data-report")
        );
        assert_eq!(
            normalize_static_page_template_reference_id(Some("  ")),
            None
        );
    }

    #[test]
    fn template_reference_infers_enabled_templates_from_intent() {
        assert_eq!(
            infer_static_page_template_reference_id("做一个运营监控看板"),
            Some("dashboard")
        );
        assert_eq!(
            infer_static_page_template_reference_id("整理接口交接文档和验收步骤"),
            Some("docs-page")
        );
        assert_eq!(
            infer_static_page_template_reference_id("生成经营分析报告和 KPI 图表"),
            Some("data-report")
        );
        assert_eq!(infer_static_page_template_reference_id("随便聊聊"), None);
    }

    #[test]
    fn template_reference_resolves_enabled_and_paused_tracks() {
        let reference = resolve_static_page_template_reference(Some("DATA-REPORT"))
            .expect("enabled template should parse")
            .expect("enabled template should resolve");
        assert_eq!(reference.id, "data-report");
        assert_eq!(reference.style_direction, "data-command");

        assert!(
            resolve_static_page_template_reference(Some("document-template-abc"))
                .expect("document template references are delegated")
                .is_none()
        );

        let paused = resolve_static_page_template_reference(Some("video-hyperframes"))
            .expect_err("paused video track should be rejected");
        assert_eq!(paused.payload.code, "static_page_template_reference_paused");
    }

    #[test]
    fn template_design_reference_keeps_safe_contract() {
        let reference = resolve_static_page_template_reference(Some("docs-page"))
            .expect("enabled template should parse")
            .expect("enabled template should resolve");
        let design = static_page_template_design_reference(reference);

        assert_eq!(design["source"], json!("html-anything"));
        assert_eq!(design["templateId"], json!("docs-page"));
        assert_eq!(
            design["providerPolicy"]["providerOutput"],
            json!("structured_static_page_draft_json")
        );
        assert!(design["providerPolicy"]["forbiddenOutput"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == "raw_html")));
        assert!(design["guardrails"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item
                    == "model output must become structured draft data, not raw final HTML")));
    }
}
