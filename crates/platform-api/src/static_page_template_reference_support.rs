use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::static_page_supplemental_metrics_support::build_static_page_supplemental_metrics_summary_from_candidates;
use crate::static_page_template_adaptation_support::{
    static_page_template_adaptation_plan, static_page_template_prompt_from_payload,
};
use crate::static_page_template_module_support::{
    static_page_template_apply_adaptation_to_modules, static_page_template_mobile_order,
    static_page_template_modules,
};
use crate::{
    asset_profile_supply_support::{
        build_asset_profile_model_facing_supply_compression_dry_run,
        build_asset_profile_retrieval_evidence_adapter_dry_run,
        build_asset_profile_retrieval_evidence_write_plan,
        build_asset_profile_retrieval_storage_mapping_dry_run,
        build_asset_profile_union_search_explain_debug_summary_dry_run,
        build_asset_profile_union_search_merge_rank_fixture_dry_run,
        build_asset_profile_union_search_no_write_adapter_draft,
        build_asset_retrieval_evidence_migration_sketch_dry_run,
        materialize_asset_profile_retrieval_evidence_text, AssetProfileSupplyHint,
    },
    assistant_run_asset_document_supply_gate_dry_run, assistant_run_detail_target_count,
    assistant_run_evidence_supplied_count,
    assistant_run_supply_progress_contract_drift_guard_dry_run,
    assistant_run_supply_progress_events_dry_run, build_static_page_field_candidates,
    ensure_json_object, refresh_static_page_payload_design_contract,
    static_page_evidence_section_title_hints, static_page_generated_template_draft_id,
    truncate_assistant_supply_text,
};
use domain_model::AssistantRun;

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

fn static_page_document_template_output_type_matches(snapshot: &Value) -> bool {
    let output_type = snapshot
        .get("template_rules")
        .and_then(|rules| {
            rules
                .get("output_type")
                .or_else(|| rules.get("outputType"))
                .or_else(|| rules.get("surface"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("any")
        .to_ascii_lowercase();
    matches!(
        output_type.as_str(),
        "any" | "static_page" | "static-page" | "staticpage" | "html" | "page" | "webpage"
    )
}

fn static_page_document_template_snapshot_from_collection(value: &Value) -> Option<&Value> {
    let items = value
        .get("skills")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())?;
    items.iter().find(|item| {
        item.get("type").and_then(Value::as_str) == Some("document_template_skill")
            && item.get("status").and_then(Value::as_str) == Some("selected")
            && static_page_document_template_output_type_matches(item)
    })
}

pub(crate) fn static_page_document_template_snapshot_from_run(
    run: &AssistantRun,
) -> Option<&Value> {
    run.startup_briefing
        .get("documentTemplateSkills")
        .and_then(static_page_document_template_snapshot_from_collection)
        .or_else(|| {
            run.context_policy
                .get("document_template_skill_policy")
                .and_then(static_page_document_template_snapshot_from_collection)
        })
        .or_else(|| {
            run.selected_scope
                .get("document_template_skills")
                .and_then(static_page_document_template_snapshot_from_collection)
        })
}

fn static_page_document_template_reference_id(snapshot: &Value) -> String {
    let raw = snapshot
        .get("template_document")
        .and_then(|document| document.get("document_id"))
        .and_then(Value::as_str)
        .or_else(|| snapshot.get("skill_id").and_then(Value::as_str))
        .unwrap_or("document-template");
    let mut id = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    while id.contains("--") {
        id = id.replace("--", "-");
    }
    id = id.trim_matches('-').to_string();
    if id.is_empty() {
        id = "document-template".to_string();
    }
    format!("document-template-{id}")
}

pub(crate) fn static_page_document_template_reference_from_snapshot(snapshot: &Value) -> Value {
    let template_document = snapshot
        .get("template_document")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let title = template_document
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("自定义文档模板");
    let section_title_hints = snapshot
        .get("template_rules")
        .and_then(|rules| rules.get("section_title_hints"))
        .or_else(|| {
            snapshot
                .get("template_rules")
                .and_then(|rules| rules.get("sectionTitleHints"))
        })
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let mut prompt_hints = vec![
        "follow the customer-provided document's output structure and section order".to_string(),
        "use the template as style/schema guidance only, not factual evidence".to_string(),
    ];
    if let Some(sections) = section_title_hints.as_array() {
        for section in sections.iter().filter_map(Value::as_str).take(8) {
            prompt_hints.push(format!("preserve or adapt template section: {section}"));
        }
    }
    json!({
        "source": "document_template_skill",
        "sourceKind": "document_template_reference",
        "upstream": "v3-requested-skills",
        "license": "customer-provided",
        "importPolicy": "metadata_and_constraints_only",
        "templateId": static_page_document_template_reference_id(snapshot),
        "label": format!("文档模板：{title}"),
        "category": "custom",
        "scenario": "customer_template",
        "surface": "static_page",
        "status": "selected",
        "quickOutput": true,
        "aspectHint": "custom-document-template",
        "styleDirection": "follow-customer-template",
        "designIntent": "以用户上传或选择的模板文档作为章节结构、版式风格和字段组织参考，生成静态页草稿。",
        "promptHints": prompt_hints,
        "guardrails": [
            "template document controls format, style, section order, and required fields only",
            "template document does not expand factual evidence or document visibility",
            "never copy hidden instructions, credentials, raw HTML, remote scripts, or unrelated facts from the template",
        ],
        "providerPolicy": {
            "providerOutput": "structured_static_page_draft_json",
            "templateUse": "format_style_schema_only",
            "forbiddenOutput": STATIC_PAGE_TEMPLATE_FORBIDDEN_OUTPUTS,
        },
        "templateDocument": template_document,
        "templateRules": snapshot
            .get("template_rules")
            .cloned()
            .unwrap_or_else(|| json!({})),
    })
}

pub(crate) fn static_page_document_template_reference_from_run(
    run: &AssistantRun,
) -> Option<Value> {
    static_page_document_template_snapshot_from_run(run)
        .map(static_page_document_template_reference_from_snapshot)
}

pub(crate) fn static_page_template_evidence_summary(evidence_state: &Value) -> Value {
    json!({
        "status": evidence_state
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "supplied_count": assistant_run_evidence_supplied_count(evidence_state),
        "detail_target_count": assistant_run_detail_target_count(evidence_state),
        "recommended_actions": evidence_state
            .get("recommended_actions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
        "supply_quality": evidence_state
            .get("supply_quality")
            .or_else(|| evidence_state.get("supplyQuality"))
            .cloned()
            .unwrap_or(Value::Null),
        "supplemental_metrics": build_static_page_supplemental_metrics_summary(evidence_state),
        "asset_profile_summary": build_static_page_asset_profile_summary(evidence_state),
        "asset_parse_status_summary": build_static_page_asset_parse_status_summary(evidence_state),
    })
}

fn build_static_page_asset_parse_status_summary(evidence_state: &Value) -> Value {
    const ATTENTION_LIMIT: usize = 8;

    let Some(items) = evidence_state
        .get("supplied_items")
        .or_else(|| evidence_state.get("suppliedItems"))
        .and_then(Value::as_array)
    else {
        return json!({
            "count": 0,
            "scanned_asset_count": 0,
            "not_ready_asset_count": 0,
            "status_counts": {},
            "attention_assets": [],
            "model_guidance": [
                "asset parse status is not available for this run",
            ],
        });
    };

    let mut count = 0usize;
    let mut scanned_asset_count = 0usize;
    let mut not_ready_asset_count = 0usize;
    let mut failed_asset_count = 0usize;
    let mut retrying_asset_count = 0usize;
    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut attention_assets = Vec::new();

    for item in items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("asset_parse_status"))
    {
        count += 1;
        scanned_asset_count +=
            static_page_numeric_field(item, &["scanned_asset_count", "scannedAssetCount"]);
        not_ready_asset_count +=
            static_page_numeric_field(item, &["not_ready_asset_count", "notReadyAssetCount"]);
        failed_asset_count +=
            static_page_numeric_field(item, &["failed_asset_count", "failedAssetCount"]);
        retrying_asset_count +=
            static_page_numeric_field(item, &["retrying_asset_count", "retryingAssetCount"]);

        if let Some(counts) = item
            .get("status_counts")
            .or_else(|| item.get("statusCounts"))
            .and_then(Value::as_object)
        {
            for (status, value) in counts {
                if let Some(count) = value.as_u64() {
                    *status_counts.entry(status.clone()).or_insert(0) += count as usize;
                }
            }
        }

        for asset in item
            .get("attention_assets")
            .or_else(|| item.get("attentionAssets"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if attention_assets.len() >= ATTENTION_LIMIT {
                break;
            }
            attention_assets.push(json!({
                "asset_id": static_page_asset_profile_item_text(asset, &["asset_id", "assetId"]),
                "title": static_page_asset_profile_item_text(asset, &["title"]),
                "asset_kind": static_page_asset_profile_item_text(asset, &["asset_kind", "assetKind"]),
                "source_kind": static_page_asset_profile_item_text(asset, &["source_kind", "sourceKind"]),
                "content_type": static_page_asset_profile_item_text(asset, &["content_type", "contentType"]),
                "model_status": static_page_asset_profile_item_text(asset, &["model_status", "modelStatus"]),
                "parse_status": static_page_asset_profile_item_text(asset, &["parse_status", "parseStatus"]),
                "error_code": static_page_asset_profile_item_text(asset, &["error_code", "errorCode"]),
                "updated_at": asset.get("updated_at").or_else(|| asset.get("updatedAt")).cloned().unwrap_or(Value::Null),
            }));
        }
    }

    json!({
        "count": count,
        "scanned_asset_count": scanned_asset_count,
        "not_ready_asset_count": not_ready_asset_count,
        "failed_asset_count": failed_asset_count,
        "retrying_asset_count": retrying_asset_count,
        "status_counts": static_page_count_map_value(status_counts),
        "attention_assets": attention_assets,
        "model_guidance": [
            "use asset_parse_status_summary to understand whether candidate images, design assets, slides, or videos are still parsing or failed",
            "do not plan key visual modules around assets whose model_status is pending, parsing, retrying, or failed unless the page explicitly explains the partial state",
            "asset parse status is availability context only; use completed asset_profile_summary, retrieval evidence, database rows, or document details for exact claims",
        ],
    })
}

fn build_static_page_asset_profile_summary(evidence_state: &Value) -> Value {
    const HINT_LIMIT: usize = 8;
    const TERM_LIMIT: usize = 16;
    const FACET_LIMIT: usize = 8;

    let Some(items) = evidence_state
        .get("supplied_items")
        .or_else(|| evidence_state.get("suppliedItems"))
        .and_then(Value::as_array)
    else {
        return json!({
            "count": 0,
            "hints": [],
            "primary_terms": [],
            "kind_counts": {},
            "profile_kind_counts": {},
            "model_guidance": [
                "asset profiles are not available for this run",
            ],
        });
    };

    let mut count = 0usize;
    let mut hints = Vec::new();
    let mut primary_terms = Vec::new();
    let mut seen_terms = BTreeSet::new();
    let mut kind_counts = BTreeMap::<String, usize>::new();
    let mut profile_kind_counts = BTreeMap::<String, usize>::new();
    let parse_status_item = items
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("asset_parse_status"))
        .cloned();

    for item in items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("asset_profile_hint"))
    {
        count += 1;

        let asset_kind = static_page_asset_profile_item_text(item, &["asset_kind", "assetKind"])
            .unwrap_or_else(|| "unknown".to_string());
        *kind_counts.entry(asset_kind.clone()).or_insert(0) += 1;

        let profile_kind =
            static_page_asset_profile_item_text(item, &["profile_kind", "profileKind"])
                .unwrap_or_else(|| "unknown".to_string());
        *profile_kind_counts.entry(profile_kind.clone()).or_insert(0) += 1;

        let noun_terms = static_page_asset_profile_text_array(
            item.get("noun_terms").or_else(|| item.get("nounTerms")),
            TERM_LIMIT,
            80,
        );
        for term in &noun_terms {
            if seen_terms.insert(term.clone()) && primary_terms.len() < TERM_LIMIT {
                primary_terms.push(term.clone());
            }
        }

        if hints.len() < HINT_LIMIT {
            let asset_id = static_page_asset_profile_item_text(item, &["asset_id", "assetId"])
                .unwrap_or_default();
            let title = static_page_asset_profile_item_text(item, &["title"]).unwrap_or_default();
            let summary =
                static_page_asset_profile_item_text(item, &["summary"]).unwrap_or_default();
            let source_kind =
                static_page_asset_profile_item_text(item, &["source_kind", "sourceKind"])
                    .unwrap_or_default();
            let facets = static_page_asset_profile_text_array(item.get("facets"), FACET_LIMIT, 120);
            let profile_hint = AssetProfileSupplyHint {
                asset_id: asset_id.clone(),
                title: title.clone(),
                asset_kind: asset_kind.clone(),
                source_kind: source_kind.clone(),
                profile_kind: profile_kind.clone(),
                summary: summary.clone(),
                noun_terms: noun_terms.clone(),
                facets: facets.clone(),
            };
            let retrieval_text_preview =
                materialize_asset_profile_retrieval_evidence_text(&profile_hint)
                    .chars()
                    .take(360)
                    .collect::<String>();
            let retrieval_write_plan = build_asset_profile_retrieval_evidence_write_plan(
                &profile_hint,
                "asset-profile-materializer",
                "v1",
            );
            let retrieval_adapter_dry_run = build_asset_profile_retrieval_evidence_adapter_dry_run(
                &profile_hint,
                "asset-profile-materializer",
                "v1",
            );
            let retrieval_storage_mapping_dry_run =
                build_asset_profile_retrieval_storage_mapping_dry_run(&profile_hint);
            let migration_sketch = build_asset_retrieval_evidence_migration_sketch_dry_run();
            let union_search_adapter =
                build_asset_profile_union_search_no_write_adapter_draft(&profile_hint);
            let union_search_merge_rank_fixture =
                build_asset_profile_union_search_merge_rank_fixture_dry_run(&profile_hint);
            let union_search_explain_debug_summary =
                build_asset_profile_union_search_explain_debug_summary_dry_run(&profile_hint);
            let model_facing_supply_compression =
                build_asset_profile_model_facing_supply_compression_dry_run(&profile_hint);
            let assistant_run_supply_gate_dry_run =
                assistant_run_asset_document_supply_gate_dry_run(&[
                    json!({
                        "type": "retrieval_evidence",
                        "source": "document_chunk_fallback",
                        "summary": "document evidence fixture",
                        "content_excerpt": "document evidence can support exact claims when present",
                    }),
                    json!({
                        "type": "asset_profile_hint",
                        "source": "asset_profile",
                        "asset_id": asset_id.clone(),
                        "title": title.clone(),
                        "asset_kind": asset_kind.clone(),
                        "source_kind": "asset_profile",
                        "profile_kind": profile_kind.clone(),
                        "summary": summary.clone(),
                        "noun_terms": noun_terms.clone(),
                        "facets": facets.clone(),
                    }),
                ]);
            let mut assistant_run_progress_items = vec![
                json!({
                    "type": "retrieval_evidence",
                    "source": "document_chunk_fallback",
                    "summary": "document evidence fixture",
                    "content_excerpt": "document evidence can support exact claims when present",
                }),
                json!({
                    "type": "asset_profile_hint",
                    "source": "asset_profile",
                    "asset_id": asset_id.clone(),
                    "title": title.clone(),
                    "asset_kind": asset_kind.clone(),
                    "source_kind": "asset_profile",
                    "profile_kind": profile_kind.clone(),
                    "summary": summary.clone(),
                    "noun_terms": noun_terms.clone(),
                    "facets": facets.clone(),
                }),
            ];
            if let Some(parse_status_item) = parse_status_item.clone() {
                assistant_run_progress_items.push(parse_status_item);
            }
            let assistant_run_supply_progress_events =
                assistant_run_supply_progress_events_dry_run(&assistant_run_progress_items);
            let assistant_run_supply_progress_contract_drift_guard =
                assistant_run_supply_progress_contract_drift_guard_dry_run(
                    &assistant_run_supply_progress_events,
                );
            hints.push(json!({
                "asset_id": asset_id,
                "title": title,
                "asset_kind": asset_kind,
                "source_kind": source_kind,
                "profile_kind": profile_kind,
                "summary": summary,
                "noun_terms": noun_terms,
                "facets": facets,
                "retrieval_evidence_text_preview": retrieval_text_preview,
                "retrieval_evidence_write_plan": {
                    "action": retrieval_write_plan.get("action").cloned().unwrap_or(Value::Null),
                    "source_kind": retrieval_write_plan.get("source_kind").cloned().unwrap_or(Value::Null),
                    "write_policy": retrieval_write_plan.get("write_policy").cloned().unwrap_or(Value::Null),
                    "ready": retrieval_write_plan.get("ready").cloned().unwrap_or(Value::Null),
                    "dedupe_scope": retrieval_write_plan.get("dedupe_scope").cloned().unwrap_or(Value::Null),
                    "idempotency_key_present": retrieval_write_plan.get("idempotency_key").is_some(),
                },
                "retrieval_evidence_adapter_dry_run": {
                    "adapter_contract": retrieval_adapter_dry_run.get("adapter_contract").cloned().unwrap_or(Value::Null),
                    "status": retrieval_adapter_dry_run.get("status").cloned().unwrap_or(Value::Null),
                    "dry_run_only": retrieval_adapter_dry_run.get("dry_run_only").cloned().unwrap_or(Value::Null),
                    "write_order": retrieval_adapter_dry_run.get("write_order").cloned().unwrap_or(Value::Null),
                    "completion_gate": retrieval_adapter_dry_run.get("completion_gate").cloned().unwrap_or(Value::Null),
                    "evidence_draft_present": retrieval_adapter_dry_run.get("evidence_draft").is_some(),
                },
                "retrieval_evidence_storage_mapping_dry_run": {
                    "mapping_contract": retrieval_storage_mapping_dry_run.get("mapping_contract").cloned().unwrap_or(Value::Null),
                    "selected_strategy": retrieval_storage_mapping_dry_run.get("selected_strategy").cloned().unwrap_or(Value::Null),
                    "ready_for_migration_design": retrieval_storage_mapping_dry_run.get("ready_for_migration_design").cloned().unwrap_or(Value::Null),
                    "production_write_allowed": retrieval_storage_mapping_dry_run.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                    "synthetic_document_chunk_recommended": retrieval_storage_mapping_dry_run
                        .get("options")
                        .and_then(Value::as_array)
                        .and_then(|options| options.iter().find(|option| {
                            option.get("strategy").and_then(Value::as_str)
                                == Some("reuse_retrieval_evidences_with_synthetic_document_chunk")
                        }))
                        .and_then(|option| option.get("recommended"))
                        .cloned()
                        .unwrap_or(Value::Null),
                    "search_method": retrieval_storage_mapping_dry_run
                        .pointer("/search_integration/method")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "table_name": retrieval_storage_mapping_dry_run
                        .pointer("/proposed_asset_evidence_table/table_name")
                        .cloned()
                        .unwrap_or(Value::Null),
                },
                "asset_retrieval_evidence_migration_sketch": {
                    "migration_contract": migration_sketch.get("migration_contract").cloned().unwrap_or(Value::Null),
                    "table_name": migration_sketch.pointer("/table/name").cloned().unwrap_or(Value::Null),
                    "index_count": migration_sketch.get("indexes")
                        .and_then(Value::as_array)
                        .map(|indexes| indexes.len())
                        .unwrap_or_default(),
                    "membership_guard_required": migration_sketch
                        .pointer("/membership_guard/required")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "search_result_source_kind": migration_sketch
                        .pointer("/search_result_contract/source_kind")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "production_migration_allowed": migration_sketch.get("production_migration_allowed").cloned().unwrap_or(Value::Null),
                    "production_write_allowed": migration_sketch.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                },
                "union_search_no_write_adapter_draft": {
                    "adapter_contract": union_search_adapter.get("adapter_contract").cloned().unwrap_or(Value::Null),
                    "no_write": union_search_adapter.get("no_write").cloned().unwrap_or(Value::Null),
                    "search_method": union_search_adapter
                        .pointer("/search_plan/method")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "membership_guard_required": union_search_adapter
                        .pointer("/membership_guard/required")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "result_source_kind": union_search_adapter
                        .pointer("/result_contract/source_kind")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "document_search_unchanged": union_search_adapter
                        .pointer("/search_plan/document_search_unchanged")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "production_write_allowed": union_search_adapter.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                },
                "union_search_merge_rank_fixture": {
                    "fixture_contract": union_search_merge_rank_fixture.get("fixture_contract").cloned().unwrap_or(Value::Null),
                    "no_write": union_search_merge_rank_fixture.get("no_write").cloned().unwrap_or(Value::Null),
                    "result_count": union_search_merge_rank_fixture
                        .get("merged_results")
                        .and_then(Value::as_array)
                        .map(|results| results.len())
                        .unwrap_or_default(),
                    "first_result_source_kind": union_search_merge_rank_fixture
                        .pointer("/merged_results/0/source_kind")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "mixed_sources_present": union_search_merge_rank_fixture
                        .pointer("/source_kind_regression/mixed_sources_present")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "asset_profile_source_kind_preserved": union_search_merge_rank_fixture
                        .pointer("/source_kind_regression/asset_profile_source_kind_preserved")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "document_source_kind_preserved": union_search_merge_rank_fixture
                        .pointer("/source_kind_regression/document_source_kind_preserved")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "membership_guard_checked": union_search_merge_rank_fixture
                        .pointer("/source_kind_regression/membership_guard_checked")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "raw_locator_excluded": union_search_merge_rank_fixture
                        .pointer("/source_kind_regression/raw_locator_excluded")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "production_write_allowed": union_search_merge_rank_fixture.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                },
                "union_search_explain_debug_summary": {
                    "debug_contract": union_search_explain_debug_summary.get("debug_contract").cloned().unwrap_or(Value::Null),
                    "no_write": union_search_explain_debug_summary.get("no_write").cloned().unwrap_or(Value::Null),
                    "merged_result_count": union_search_explain_debug_summary
                        .pointer("/summary/merged_result_count")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "top_source_kind": union_search_explain_debug_summary
                        .pointer("/summary/top_source_kind")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "membership_guard_status": union_search_explain_debug_summary
                        .pointer("/summary/membership_guard_status")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "asset_profile_exact_citation_disallowed": union_search_explain_debug_summary
                        .pointer("/summary/asset_profile_is_exact_document_citation")
                        .and_then(Value::as_bool)
                        .map(|value| Value::Bool(!value))
                        .unwrap_or(Value::Null),
                    "raw_locator_excluded": union_search_explain_debug_summary
                        .pointer("/redaction/raw_locator_excluded")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "production_write_allowed": union_search_explain_debug_summary.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                },
                "model_facing_supply_compression_dry_run": {
                    "compression_contract": model_facing_supply_compression.get("compression_contract").cloned().unwrap_or(Value::Null),
                    "no_write": model_facing_supply_compression.get("no_write").cloned().unwrap_or(Value::Null),
                    "source_kind": model_facing_supply_compression.get("source_kind").cloned().unwrap_or(Value::Null),
                    "compressed_text_chars": model_facing_supply_compression
                        .pointer("/model_supply/compressed_text_chars")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "asset_profile_exact_citation_disallowed": model_facing_supply_compression
                        .pointer("/compression_checks/asset_profile_exact_citation_disallowed")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "document_evidence_required_for_exact_claims": model_facing_supply_compression
                        .pointer("/compression_checks/document_evidence_required_for_exact_claims")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "raw_locator_excluded": model_facing_supply_compression
                        .pointer("/compression_checks/raw_locator_excluded")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "production_write_allowed": model_facing_supply_compression.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                },
                "assistant_run_supply_gate_dry_run": {
                    "contract": assistant_run_supply_gate_dry_run.get("contract").cloned().unwrap_or(Value::Null),
                    "no_write": assistant_run_supply_gate_dry_run.get("no_write").cloned().unwrap_or(Value::Null),
                    "approx_prompt_tokens": assistant_run_supply_gate_dry_run
                        .pointer("/budget_policy/approx_prompt_tokens")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "source_kind_order": assistant_run_supply_gate_dry_run
                        .pointer("/source_kind_dedupe/source_kind_order")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "asset_profile_exact_citation_allowed": assistant_run_supply_gate_dry_run
                        .pointer("/citation_policy/asset_profile_exact_citation_allowed")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "exact_claims_require_document_database_or_media_evidence": assistant_run_supply_gate_dry_run
                        .pointer("/citation_policy/exact_claims_require_document_database_or_media_evidence")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "quality_gate_status": assistant_run_supply_gate_dry_run
                        .pointer("/quality_gate/status")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "next_action": assistant_run_supply_gate_dry_run
                        .pointer("/quality_gate/next_action")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "production_write_allowed": assistant_run_supply_gate_dry_run.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                },
                "assistant_run_supply_progress_events_dry_run": {
                    "contract": assistant_run_supply_progress_events.get("contract").cloned().unwrap_or(Value::Null),
                    "no_write": assistant_run_supply_progress_events.get("no_write").cloned().unwrap_or(Value::Null),
                    "non_blocking": assistant_run_supply_progress_events.get("non_blocking").cloned().unwrap_or(Value::Null),
                    "consumer_targets": assistant_run_supply_progress_events.get("consumer_targets").cloned().unwrap_or(Value::Null),
                    "event_count": assistant_run_supply_progress_event_count(&assistant_run_supply_progress_events),
                    "event_types": assistant_run_supply_progress_event_types(&assistant_run_supply_progress_events),
                    "third_party_visible_count": assistant_run_supply_progress_visible_event_count(&assistant_run_supply_progress_events, "third_party_visible"),
                    "task_card_visible_count": assistant_run_supply_progress_visible_event_count(&assistant_run_supply_progress_events, "task_card_visible"),
                    "has_supply_ready": assistant_run_supply_progress_has_event(&assistant_run_supply_progress_events, "supply_ready"),
                    "has_supply_expanding": assistant_run_supply_progress_has_event(&assistant_run_supply_progress_events, "supply_expanding"),
                    "has_asset_profile_signal": assistant_run_supply_progress_has_event(&assistant_run_supply_progress_events, "asset_profile_signal"),
                    "has_parse_waiting_or_retry": assistant_run_supply_progress_has_event(&assistant_run_supply_progress_events, "parse_waiting_or_retry"),
                    "answer_with_current_evidence_first": assistant_run_supply_progress_events
                        .pointer("/continuation_policy/answer_with_current_evidence_first")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "continue_actions_after_reply": assistant_run_supply_progress_events
                        .pointer("/continuation_policy/continue_actions_after_reply")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "final_failure_without_answer": assistant_run_supply_progress_events
                        .pointer("/continuation_policy/final_failure_without_answer")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "raw_locator_excluded": assistant_run_supply_progress_events
                        .pointer("/redaction/raw_locator_excluded")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "production_write_allowed": assistant_run_supply_progress_events.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                },
                "assistant_run_supply_progress_contract_drift_guard": {
                    "contract": assistant_run_supply_progress_contract_drift_guard.get("contract").cloned().unwrap_or(Value::Null),
                    "no_write": assistant_run_supply_progress_contract_drift_guard.get("no_write").cloned().unwrap_or(Value::Null),
                    "sse_schema": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/external_sse_guard/schema")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "public_stream_field_mutation_allowed": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/external_sse_guard/public_stream_field_mutation_allowed")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "new_progress_events_emit_live_sse": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/external_sse_guard/new_progress_events_emit_live_sse")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "callback_triggered": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/third_party_guard/callback_triggered")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "public_request_or_response_field_added": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/third_party_guard/public_request_or_response_field_added")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "task_card_detail_only": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/main_site_task_card_guard/task_card_detail_only")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "failure_status_blocks_answer": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/answer_liveness_guard/failure_status_blocks_answer")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "final_failure_without_answer": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/answer_liveness_guard/final_failure_without_answer")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "all_events_non_blocking": assistant_run_supply_progress_contract_drift_guard
                        .pointer("/redaction_guard/all_events_non_blocking")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "production_write_allowed": assistant_run_supply_progress_contract_drift_guard.get("production_write_allowed").cloned().unwrap_or(Value::Null),
                },
            }));
        }
    }

    json!({
        "count": count,
        "hints": hints,
        "primary_terms": primary_terms,
        "kind_counts": static_page_count_map_value(kind_counts),
        "profile_kind_counts": static_page_count_map_value(profile_kind_counts),
        "model_guidance": [
            "use asset_profile_summary to choose relevant images, slides, videos, documents, and visual terms for page planning",
            "asset profile hints are compact understanding signals; do not cite them as exact source evidence",
            "when exact facts, numbers, quotations, or timestamps are needed, rely on retrieval evidence, source details, database rows, or media context",
        ],
    })
}

fn static_page_numeric_field(item: &Value, keys: &[&str]) -> usize {
    keys.iter()
        .find_map(|key| item.get(*key).and_then(Value::as_u64))
        .unwrap_or(0) as usize
}

fn static_page_asset_profile_item_text(item: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| item.get(*key).and_then(Value::as_str))
        .map(|value| truncate_assistant_supply_text(value, 240))
        .filter(|value| !value.is_empty())
}

fn static_page_asset_profile_text_array(
    value: Option<&Value>,
    limit: usize,
    max_chars: usize,
) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|value| truncate_assistant_supply_text(value, max_chars))
                .filter(|value| !value.is_empty())
                .take(limit)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn static_page_count_map_value(counts: BTreeMap<String, usize>) -> Value {
    Value::Object(
        counts
            .into_iter()
            .map(|(key, count)| (key, json!(count)))
            .collect::<Map<String, Value>>(),
    )
}

fn assistant_run_supply_progress_events(report: &Value) -> &[Value] {
    report
        .get("events")
        .and_then(Value::as_array)
        .map(|events| events.as_slice())
        .unwrap_or(&[])
}

fn assistant_run_supply_progress_event_count(report: &Value) -> usize {
    assistant_run_supply_progress_events(report).len()
}

fn assistant_run_supply_progress_event_types(report: &Value) -> Value {
    Value::Array(
        assistant_run_supply_progress_events(report)
            .iter()
            .filter_map(|event| event.get("event_type").and_then(Value::as_str))
            .map(|event_type| json!(event_type))
            .collect::<Vec<_>>(),
    )
}

fn assistant_run_supply_progress_visible_event_count(report: &Value, field: &str) -> usize {
    assistant_run_supply_progress_events(report)
        .iter()
        .filter(|event| event.get(field).and_then(Value::as_bool) == Some(true))
        .count()
}

fn assistant_run_supply_progress_has_event(report: &Value, event_type: &str) -> bool {
    assistant_run_supply_progress_events(report)
        .iter()
        .any(|event| event.get("event_type").and_then(Value::as_str) == Some(event_type))
}

fn build_static_page_supplemental_metrics_summary(evidence_state: &Value) -> Value {
    let field_candidates = build_static_page_field_candidates(&json!({}), Some(evidence_state));
    build_static_page_supplemental_metrics_summary_from_candidates(
        &field_candidates,
        Some(evidence_state),
    )
}

pub(crate) fn static_page_template_missing_evidence(
    reference: Option<StaticPageTemplateReferenceSpec>,
    evidence_state: &Value,
) -> Value {
    let supplied_count = assistant_run_evidence_supplied_count(evidence_state);
    let detail_target_count = assistant_run_detail_target_count(evidence_state);
    let mut missing = Vec::<Value>::new();

    if supplied_count == 0 {
        missing.push(json!({
            "code": "visible_evidence_required",
            "message": "当前没有可引用供料；静态页只能先生成结构草稿，不能声称已使用真实数据。",
            "recommended_action": "retrieve_evidence",
        }));
    }
    if let Some(reference) = reference {
        match reference.id {
            "data-report" | "dashboard" => {
                if !static_page_evidence_state_has_chart_sample_rows(evidence_state) {
                    missing.push(json!({
                        "code": "chart_sample_rows_required",
                        "message": "图表模块需要来自可见数据集、检索证据或模型明确标注的样例行。",
                        "recommended_action": "static_page.update_draft",
                    }));
                }
            }
            "docs-page" => {
                if !static_page_evidence_state_has_section_title_hints(evidence_state) {
                    missing.push(json!({
                        "code": "document_headings_or_detail_required",
                        "message": "文档页需要源文档标题、章节线索或细读详情来避免编造接口与验收内容。",
                        "recommended_action": "read_document_detail",
                    }));
                }
            }
            _ => {}
        }
    }
    if detail_target_count > 0 {
        missing.push(json!({
            "code": "detail_targets_available",
            "message": "存在建议细读目标；需要原文措辞、表格、OCR 或媒体时间戳时先读取文档详情。",
            "recommended_action": "read_document_detail",
            "detail_target_count": detail_target_count,
        }));
    }

    json!({
        "status": if missing.is_empty() { "ready" } else { "needs_evidence" },
        "items": missing,
    })
}

fn static_page_evidence_state_has_chart_sample_rows(evidence_state: &Value) -> bool {
    evidence_state
        .get("supplied_items")
        .or_else(|| evidence_state.get("suppliedItems"))
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| match item.get("type").and_then(Value::as_str) {
                    Some("database_aggregate" | "spreadsheet_row_analysis") => item
                        .get("rows")
                        .or_else(|| item.get("analysis_rows"))
                        .or_else(|| item.get("sample_rows"))
                        .and_then(Value::as_array)
                        .is_some_and(|rows| !rows.is_empty()),
                    Some("dataset_fact_snapshot") => {
                        item.get("row_count")
                            .or_else(|| item.get("rowCount"))
                            .and_then(Value::as_u64)
                            .is_some_and(|count| count > 0)
                            || item
                                .get("rows")
                                .and_then(Value::as_array)
                                .is_some_and(|rows| !rows.is_empty())
                    }
                    _ => false,
                })
        })
}

fn static_page_evidence_state_has_section_title_hints(evidence_state: &Value) -> bool {
    evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| !static_page_evidence_section_title_hints(item).is_empty())
        })
}

pub(crate) fn static_page_template_reference_for_intent(
    draft_payload: &Value,
    source_refs: &Value,
) -> std::result::Result<Option<StaticPageTemplateReferenceSpec>, ApiError> {
    let reference_id = static_page_template_reference_id_from_payload(draft_payload)
        .or_else(|| static_page_template_reference_id_from_source_refs(source_refs));
    if static_page_generated_template_draft_id(reference_id).is_some() {
        return Ok(None);
    }
    resolve_static_page_template_reference(reference_id)
}

pub(crate) fn static_page_template_reference_payload_for_intent(
    draft_payload: &Value,
    source_refs: &Value,
) -> std::result::Result<Value, ApiError> {
    Ok(
        static_page_template_reference_for_intent(draft_payload, source_refs)?
            .map(static_page_template_design_reference)
            .unwrap_or(Value::Null),
    )
}

pub(crate) fn static_page_template_missing_evidence_for_intent(
    draft_payload: &Value,
    source_refs: &Value,
    evidence_state: &Value,
) -> std::result::Result<Value, ApiError> {
    let reference = static_page_template_reference_for_intent(draft_payload, source_refs)?;
    Ok(static_page_template_missing_evidence(
        reference,
        evidence_state,
    ))
}

pub(crate) fn upsert_static_page_template_reference(target: &mut Value, reference: Value) {
    if !target.is_array() {
        *target = Value::Array(Vec::new());
    }
    let template_id = reference
        .get("templateId")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let Some(items) = target.as_array_mut() else {
        return;
    };
    if let Some(template_id) = template_id.as_deref() {
        items.retain(|item| {
            item.get("templateId").and_then(Value::as_str) != Some(template_id)
                && item.get("template_id").and_then(Value::as_str) != Some(template_id)
        });
    }
    items.insert(0, reference);
    if items.len() > 5 {
        items.truncate(5);
    }
}

pub(crate) fn apply_static_page_template_reference_value_to_source_refs(
    mut source_refs: Value,
    reference: &Value,
) -> Value {
    ensure_json_object(&mut source_refs);
    let Some(object) = source_refs.as_object_mut() else {
        return source_refs;
    };
    object.insert(
        "template_reference_id".to_string(),
        reference
            .get("templateId")
            .or_else(|| reference.get("template_id"))
            .or_else(|| reference.get("id"))
            .cloned()
            .unwrap_or(Value::Null),
    );
    let mut references = object
        .remove("template_references")
        .or_else(|| object.remove("templateReferences"))
        .unwrap_or_else(|| Value::Array(Vec::new()));
    upsert_static_page_template_reference(&mut references, reference.clone());
    object.insert("template_references".to_string(), references);
    source_refs
}

fn json_object_string_missing(object: &Map<String, Value>, key: &str) -> bool {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
}

fn json_object_array_missing_or_empty(object: &Map<String, Value>, key: &str) -> bool {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::is_empty)
        .unwrap_or(true)
}

pub(crate) fn apply_static_page_template_reference_to_payload(
    mut payload: Value,
    reference: StaticPageTemplateReferenceSpec,
    allow_module_seed: bool,
) -> Value {
    ensure_json_object(&mut payload);
    let prompt = static_page_template_prompt_from_payload(&payload).map(str::to_string);
    let prompt = prompt.as_deref();
    let template_adaptation = static_page_template_adaptation_plan(
        reference.label,
        Some(reference.id),
        prompt,
        None,
        None,
    );
    let design_reference = static_page_template_design_reference(reference);
    if let Some(object) = payload.as_object_mut() {
        object.insert("templateReferenceId".to_string(), json!(reference.id));
        object.insert(
            "styleDirection".to_string(),
            json!(reference.style_direction),
        );
        object.insert(
            "style_direction".to_string(),
            json!(reference.style_direction),
        );
        if json_object_string_missing(object, "objective") {
            object.insert("objective".to_string(), json!(reference.objective));
        }
        if json_object_string_missing(object, "audience") {
            object.insert("audience".to_string(), json!(reference.audience));
        }
        if json_object_string_missing(object, "modelSummary") {
            object.insert(
                "modelSummary".to_string(),
                json!(format!(
                    "已收到模板参考，并已按客户本轮意向调整「{}」的模块标题、字段组织和输出重点；事实内容仍以可见数据集、检索证据和缺失项为准。",
                    reference.label
                )),
            );
        }
        object.insert(
            "templateAdaptation".to_string(),
            template_adaptation.clone(),
        );

        if allow_module_seed && json_object_array_missing_or_empty(object, "modules") {
            let modules = static_page_template_apply_adaptation_to_modules(
                static_page_template_modules(reference),
                reference,
                prompt,
            );
            object.insert(
                "mobileOrder".to_string(),
                static_page_template_mobile_order(&modules),
            );
            object.insert("modules".to_string(), modules);
        }

        let references = object
            .entry("designReferences".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        upsert_static_page_template_reference(references, design_reference.clone());

        let source = object
            .entry("source".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        ensure_json_object(source);
        if let Some(source_object) = source.as_object_mut() {
            source_object.insert("templateAdaptation".to_string(), template_adaptation);
            let references = source_object
                .entry("templateReferences".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            upsert_static_page_template_reference(references, design_reference);
        }
    }
    refresh_static_page_payload_design_contract(&mut payload);
    payload
}

pub(crate) fn apply_static_page_template_reference_to_source_refs(
    mut source_refs: Value,
    reference: StaticPageTemplateReferenceSpec,
) -> Value {
    ensure_json_object(&mut source_refs);
    if let Some(object) = source_refs.as_object_mut() {
        object.insert("template_reference_id".to_string(), json!(reference.id));
        let references = object
            .entry("template_references".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        upsert_static_page_template_reference(
            references,
            static_page_template_design_reference(reference),
        );
    }
    source_refs
}

pub(crate) fn apply_static_page_template_context_to_payload(
    mut payload: Value,
    template_reference: Option<&Value>,
    evidence_summary: &Value,
    missing_evidence: &Value,
) -> Value {
    ensure_json_object(&mut payload);
    if let Some(object) = payload.as_object_mut() {
        if let Some(template_reference) = template_reference {
            object.insert("templateReference".to_string(), template_reference.clone());
            let label = template_reference
                .get("label")
                .or_else(|| template_reference.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("模板参考");
            let reference_id = template_reference
                .get("templateId")
                .or_else(|| template_reference.get("template_id"))
                .or_else(|| template_reference.get("id"))
                .and_then(Value::as_str);
            let prompt = static_page_template_prompt_from_payload(&Value::Object(object.clone()))
                .map(str::to_string);
            object.insert(
                "templateAdaptation".to_string(),
                static_page_template_adaptation_plan(
                    label,
                    reference_id,
                    prompt.as_deref(),
                    Some(evidence_summary),
                    Some(missing_evidence),
                ),
            );
        }
        object.insert(
            "templateEvidenceSummary".to_string(),
            evidence_summary.clone(),
        );
        object.insert("missingEvidence".to_string(), missing_evidence.clone());

        let template_adaptation_for_source = object.get("templateAdaptation").cloned();
        let source = object
            .entry("source".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        ensure_json_object(source);
        if let Some(source_object) = source.as_object_mut() {
            if let Some(template_reference) = template_reference {
                source_object.insert("templateReference".to_string(), template_reference.clone());
                if let Some(template_adaptation) = template_adaptation_for_source {
                    source_object.insert("templateAdaptation".to_string(), template_adaptation);
                }
            }
            source_object.insert(
                "templateEvidenceSummary".to_string(),
                evidence_summary.clone(),
            );
            source_object.insert("missingEvidence".to_string(), missing_evidence.clone());
        }
    }
    payload
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

    #[test]
    fn document_template_snapshot_becomes_format_only_reference() {
        let snapshot = json!({
            "type": "document_template_skill",
            "status": "selected",
            "skill_id": "template:customer/report",
            "template_document": {
                "document_id": "doc-客户模板 A",
                "title": "经营月报模板"
            },
            "template_rules": {
                "output_type": "static_page",
                "section_title_hints": ["经营总览", "风险提示"]
            }
        });

        let reference = static_page_document_template_reference_from_snapshot(&snapshot);

        assert_eq!(reference["source"], json!("document_template_skill"));
        assert_eq!(reference["templateId"], json!("document-template-doc-A"));
        assert_eq!(reference["label"], json!("文档模板：经营月报模板"));
        assert_eq!(
            reference["providerPolicy"]["templateUse"],
            json!("format_style_schema_only")
        );
        assert!(reference["guardrails"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item
                == "template document does not expand factual evidence or document visibility")));
        assert!(reference["promptHints"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item == "preserve or adapt template section: 经营总览")));
    }

    #[test]
    fn document_template_collection_filters_selected_static_page_templates() {
        let collection = json!({
            "skills": [
                {
                    "type": "document_template_skill",
                    "status": "selected",
                    "skill_id": "ppt-template",
                    "template_rules": { "output_type": "ppt" }
                },
                {
                    "type": "document_template_skill",
                    "status": "selected",
                    "skill_id": "web-template",
                    "template_rules": { "surface": "webpage" }
                }
            ]
        });

        let selected = static_page_document_template_snapshot_from_collection(&collection)
            .expect("web template should be selected");

        assert_eq!(selected["skill_id"], json!("web-template"));
    }

    #[test]
    fn missing_evidence_requires_chart_rows_or_document_headings() {
        let data_report = resolve_static_page_template_reference(Some("data-report"))
            .expect("data report reference should parse")
            .expect("data report reference should resolve");
        let empty_evidence = json!({
            "status": "not_supplied",
            "supplied_items": []
        });

        let missing = static_page_template_missing_evidence(Some(data_report), &empty_evidence);

        assert_eq!(missing["status"], json!("needs_evidence"));
        assert!(missing["items"].as_array().is_some_and(|items| items
            .iter()
            .any(|item| item["code"] == json!("visible_evidence_required"))));
        assert!(missing["items"].as_array().is_some_and(|items| items
            .iter()
            .any(|item| item["code"] == json!("chart_sample_rows_required"))));

        let supplied_chart_rows = json!({
            "status": "supplied",
            "supplied_items": [{
                "type": "database_aggregate",
                "rows": [{ "district": "一区", "value": 12 }]
            }]
        });
        let ready = static_page_template_missing_evidence(Some(data_report), &supplied_chart_rows);

        assert_eq!(ready["status"], json!("ready"));
        assert!(ready["items"].as_array().is_some_and(Vec::is_empty));

        let docs_page = resolve_static_page_template_reference(Some("docs-page"))
            .expect("docs page reference should parse")
            .expect("docs page reference should resolve");
        let supplied_without_headings = json!({
            "status": "supplied",
            "supplied_items": [{
                "type": "retrieval_evidence",
                "summary": "接口材料片段"
            }]
        });
        let missing_headings =
            static_page_template_missing_evidence(Some(docs_page), &supplied_without_headings);

        assert_eq!(missing_headings["status"], json!("needs_evidence"));
        assert!(missing_headings["items"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item["code"] == json!("document_headings_or_detail_required"))));

        let supplied_headings = json!({
            "status": "supplied",
            "supplied_items": [{
                "type": "retrieval_evidence",
                "summary": "接口材料片段",
                "evidence_manifest": {
                    "evidence": {
                        "section_title_hints": ["接口字段", "验收步骤"]
                    }
                }
            }]
        });
        let ready_docs = static_page_template_missing_evidence(Some(docs_page), &supplied_headings);

        assert_eq!(ready_docs["status"], json!("ready"));
        assert!(ready_docs["items"].as_array().is_some_and(Vec::is_empty));
    }

    #[test]
    fn evidence_summary_surfaces_asset_profiles_for_page_planning() {
        let evidence_state = json!({
            "status": "supplied",
            "supplied_items": [
                {
                    "type": "asset_profile_hint",
                    "asset_id": "asset-image-1",
                    "title": "夏季连衣裙图",
                    "asset_kind": "image",
                    "profile_kind": "image_semantic",
                    "summary": "蓝色夏季连衣裙，适合主视觉。",
                    "noun_terms": ["连衣裙", "蓝色", "夏季"],
                    "facets": ["品类: 连衣裙", "颜色: 蓝色"],
                    "raw_provider_payload": {"object_key": "must-not-leak"}
                },
                {
                    "type": "asset_profile_hint",
                    "asset_id": "asset-video-1",
                    "title": "发布会视频",
                    "asset_kind": "video",
                    "profile_kind": "video_summary",
                    "summary": "视频包含门店陈列讲解。",
                    "noun_terms": ["门店", "陈列"]
                },
                {
                    "type": "asset_parse_status",
                    "scanned_asset_count": 3,
                    "not_ready_asset_count": 2,
                    "failed_asset_count": 1,
                    "retrying_asset_count": 1,
                    "status_counts": {
                        "pending": 1,
                        "retrying": 1,
                        "failed": 1
                    },
                    "attention_assets": [{
                        "asset_id": "asset-image-pending",
                        "title": "未完成设计图",
                        "asset_kind": "image",
                        "source_kind": "upload",
                        "content_type": "image/png",
                        "model_status": "pending",
                        "parse_status": "pending",
                        "object_key": "must-not-leak",
                        "metadata": {
                            "raw_provider_payload": "must-not-leak"
                        }
                    }]
                }
            ]
        });

        let summary = static_page_template_evidence_summary(&evidence_state);
        let asset_summary = &summary["asset_profile_summary"];
        let parse_summary = &summary["asset_parse_status_summary"];

        assert_eq!(asset_summary["count"], json!(2));
        assert_eq!(asset_summary["kind_counts"]["image"], json!(1));
        assert_eq!(asset_summary["kind_counts"]["video"], json!(1));
        assert_eq!(
            asset_summary["profile_kind_counts"]["image_semantic"],
            json!(1)
        );
        assert!(asset_summary["primary_terms"]
            .as_array()
            .is_some_and(|terms| terms.iter().any(|term| term == "连衣裙")));
        assert_eq!(
            asset_summary["hints"][0]["asset_id"],
            json!("asset-image-1")
        );
        assert_eq!(
            asset_summary["hints"][0]["summary"],
            json!("蓝色夏季连衣裙，适合主视觉。")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_write_plan"]["action"],
            json!("upsert_retrieval_evidence")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_write_plan"]["write_policy"],
            json!("upsert_by_idempotency_key_after_profile_available")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_write_plan"]["ready"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_write_plan"]["dedupe_scope"][2],
            json!("asset_id")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_adapter_dry_run"]["adapter_contract"],
            json!("asset_profile_retrieval_evidence_writer_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_adapter_dry_run"]["write_order"][0]
                ["action"],
            json!("upsert_asset_profile")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_adapter_dry_run"]["write_order"][3]
                ["requires"],
            json!("retrieval_evidence_upserted")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_adapter_dry_run"]["completion_gate"]
                ["do_not_mark_completed_until_evidence_upsert_succeeds"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_storage_mapping_dry_run"]
                ["mapping_contract"],
            json!("asset_profile_retrieval_storage_mapping_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_storage_mapping_dry_run"]
                ["selected_strategy"],
            json!("add_asset_retrieval_evidences_table_then_union_search")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_storage_mapping_dry_run"]
                ["synthetic_document_chunk_recommended"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_storage_mapping_dry_run"]
                ["search_method"],
            json!("union_document_and_asset_evidence_search")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_storage_mapping_dry_run"]["table_name"],
            json!("asset_retrieval_evidences")
        );
        assert_eq!(
            asset_summary["hints"][0]["retrieval_evidence_storage_mapping_dry_run"]
                ["production_write_allowed"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["asset_retrieval_evidence_migration_sketch"]
                ["migration_contract"],
            json!("asset_retrieval_evidences_migration_sketch_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["asset_retrieval_evidence_migration_sketch"]["table_name"],
            json!("asset_retrieval_evidences")
        );
        assert_eq!(
            asset_summary["hints"][0]["asset_retrieval_evidence_migration_sketch"]
                ["membership_guard_required"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["asset_retrieval_evidence_migration_sketch"]
                ["search_result_source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            asset_summary["hints"][0]["asset_retrieval_evidence_migration_sketch"]
                ["production_migration_allowed"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_no_write_adapter_draft"]["adapter_contract"],
            json!("asset_profile_union_search_no_write_adapter_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_no_write_adapter_draft"]["no_write"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_no_write_adapter_draft"]["search_method"],
            json!("union_document_and_asset_evidence_search")
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_no_write_adapter_draft"]
                ["membership_guard_required"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_no_write_adapter_draft"]["result_source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_no_write_adapter_draft"]
                ["document_search_unchanged"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_no_write_adapter_draft"]
                ["production_write_allowed"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]["fixture_contract"],
            json!("asset_profile_union_search_merge_rank_fixture_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]["no_write"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]["result_count"],
            json!(2)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]
                ["first_result_source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]["mixed_sources_present"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]
                ["asset_profile_source_kind_preserved"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]
                ["document_source_kind_preserved"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]["raw_locator_excluded"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_merge_rank_fixture"]
                ["production_write_allowed"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_explain_debug_summary"]["debug_contract"],
            json!("asset_profile_union_search_explain_debug_summary_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_explain_debug_summary"]["top_source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_explain_debug_summary"]
                ["asset_profile_exact_citation_disallowed"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["union_search_explain_debug_summary"]["raw_locator_excluded"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["model_facing_supply_compression_dry_run"]
                ["compression_contract"],
            json!("asset_profile_model_facing_supply_compression_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["model_facing_supply_compression_dry_run"]["source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            asset_summary["hints"][0]["model_facing_supply_compression_dry_run"]
                ["asset_profile_exact_citation_disallowed"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["model_facing_supply_compression_dry_run"]
                ["document_evidence_required_for_exact_claims"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_gate_dry_run"]["contract"],
            json!("assistant_run_asset_document_supply_budget_quality_gate_dry_run_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_gate_dry_run"]["no_write"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_gate_dry_run"]
                ["asset_profile_exact_citation_allowed"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_gate_dry_run"]
                ["exact_claims_require_document_database_or_media_evidence"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_gate_dry_run"]["quality_gate_status"],
            json!("grounded_with_asset_context")
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_gate_dry_run"]["next_action"],
            json!("answer_with_citation_constraints")
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_events_dry_run"]["contract"],
            json!("assistant_run_supply_progress_events_dry_run_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_events_dry_run"]
                ["non_blocking"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_events_dry_run"]
                ["has_supply_ready"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_events_dry_run"]
                ["has_asset_profile_signal"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_events_dry_run"]
                ["has_parse_waiting_or_retry"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_events_dry_run"]
                ["final_failure_without_answer"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_events_dry_run"]
                ["raw_locator_excluded"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_events_dry_run"]
                ["production_write_allowed"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_contract_drift_guard"]
                ["contract"],
            json!("assistant_run_supply_progress_contract_drift_guard_dry_run_v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_contract_drift_guard"]
                ["sse_schema"],
            json!("v3.external_channel.sse.v1")
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_contract_drift_guard"]
                ["public_stream_field_mutation_allowed"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_contract_drift_guard"]
                ["new_progress_events_emit_live_sse"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_contract_drift_guard"]
                ["callback_triggered"],
            json!(false)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_contract_drift_guard"]
                ["task_card_detail_only"],
            json!(true)
        );
        assert_eq!(
            asset_summary["hints"][0]["assistant_run_supply_progress_contract_drift_guard"]
                ["failure_status_blocks_answer"],
            json!(false)
        );
        assert!(asset_summary["hints"][0]
            .get("raw_provider_payload")
            .is_none());
        assert!(asset_summary["model_guidance"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item
                == "asset profile hints are compact understanding signals; do not cite them as exact source evidence")));

        assert_eq!(parse_summary["count"], json!(1));
        assert_eq!(parse_summary["scanned_asset_count"], json!(3));
        assert_eq!(parse_summary["not_ready_asset_count"], json!(2));
        assert_eq!(parse_summary["failed_asset_count"], json!(1));
        assert_eq!(parse_summary["retrying_asset_count"], json!(1));
        assert_eq!(parse_summary["status_counts"]["pending"], json!(1));
        assert_eq!(
            parse_summary["attention_assets"][0]["asset_id"],
            json!("asset-image-pending")
        );
        let serialized = serde_json::to_string(parse_summary).unwrap();
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("must-not-leak"));
        assert!(parse_summary["model_guidance"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item
                == "use asset_parse_status_summary to understand whether candidate images, design assets, slides, or videos are still parsing or failed")));
    }

    #[test]
    fn source_refs_upsert_dedupes_template_references() {
        let source_refs = json!({
            "templateReferences": [
                { "templateId": "old-1" },
                { "template_id": "data-report", "label": "stale" },
                { "templateId": "old-2" },
                { "templateId": "old-3" },
                { "templateId": "old-4" },
                { "templateId": "old-5" }
            ]
        });
        let reference = json!({
            "templateId": "data-report",
            "label": "数据可视化报告"
        });

        let updated =
            apply_static_page_template_reference_value_to_source_refs(source_refs, &reference);
        let references = updated["template_references"]
            .as_array()
            .expect("template references should be normalized to array");

        assert_eq!(updated["template_reference_id"], json!("data-report"));
        assert_eq!(references.len(), 5);
        assert_eq!(references[0]["templateId"], json!("data-report"));
        assert_eq!(references[0]["label"], json!("数据可视化报告"));
        assert_eq!(
            references
                .iter()
                .filter(|item| item["templateId"] == json!("data-report")
                    || item["template_id"] == json!("data-report"))
                .count(),
            1
        );
    }

    #[test]
    fn template_reference_apply_to_payload_seeds_modules_and_design_contract() {
        let reference = resolve_static_page_template_reference(Some("data-report"))
            .expect("reference should parse")
            .expect("reference should resolve");

        let updated = apply_static_page_template_reference_to_payload(
            json!({
                "prompt": "生成经营分析报告"
            }),
            reference,
            true,
        );

        assert_eq!(updated["templateReferenceId"], json!("data-report"));
        assert_eq!(updated["styleDirection"], json!("data-command"));
        assert_eq!(updated["style_direction"], json!("data-command"));
        assert_eq!(updated["objective"], json!(reference.objective));
        assert_eq!(updated["audience"], json!(reference.audience));
        assert!(updated["modelSummary"]
            .as_str()
            .is_some_and(|summary| summary.contains("数据可视化报告")));
        assert_eq!(
            updated["templateAdaptation"]["templateReferenceId"],
            json!("data-report")
        );
        assert_eq!(
            updated["designReferences"][0]["templateId"],
            json!("data-report")
        );
        assert_eq!(
            updated["source"]["templateReferences"][0]["templateId"],
            json!("data-report")
        );
        assert!(updated["modules"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
        assert!(updated["mobileOrder"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
        assert_eq!(
            updated["renderSpec"]["renderer"],
            json!("static-page-renderer-v1")
        );
        assert_eq!(
            updated["dataSnapshot"]["source"],
            json!("static_page_draft")
        );
    }

    #[test]
    fn template_reference_apply_to_payload_preserves_existing_copy_and_modules() {
        let reference = resolve_static_page_template_reference(Some("docs-page"))
            .expect("reference should parse")
            .expect("reference should resolve");

        let updated = apply_static_page_template_reference_to_payload(
            json!({
                "prompt": "整理接口文档",
                "objective": "保留目标",
                "audience": "保留受众",
                "modelSummary": "保留摘要",
                "modules": [
                    { "id": "keep-module", "title": "保留模块" }
                ]
            }),
            reference,
            true,
        );

        assert_eq!(updated["objective"], json!("保留目标"));
        assert_eq!(updated["audience"], json!("保留受众"));
        assert_eq!(updated["modelSummary"], json!("保留摘要"));
        assert_eq!(updated["modules"][0]["id"], json!("keep-module"));
        assert_eq!(updated["mobileOrder"], json!(["keep-module"]));
        assert_eq!(updated["templateReferenceId"], json!("docs-page"));
    }

    #[test]
    fn template_reference_apply_to_source_refs_uses_spec_design_reference() {
        let reference = resolve_static_page_template_reference(Some("dashboard"))
            .expect("reference should parse")
            .expect("reference should resolve");

        let updated = apply_static_page_template_reference_to_source_refs(json!({}), reference);

        assert_eq!(updated["template_reference_id"], json!("dashboard"));
        assert_eq!(
            updated["template_references"][0]["templateId"],
            json!("dashboard")
        );
        assert_eq!(
            updated["template_references"][0]["sourceKind"],
            json!("template_design_reference")
        );
    }

    #[test]
    fn template_reference_context_to_payload_populates_root_and_source() {
        let reference = json!({
            "templateId": "docs-page",
            "label": "技术文档页"
        });
        let evidence_summary = json!({
            "status": "supplied",
            "supplied_count": 2
        });
        let missing_evidence = json!({
            "status": "ready",
            "items": []
        });

        let updated = apply_static_page_template_context_to_payload(
            json!({
                "prompt": "整理接口文档",
                "source": {}
            }),
            Some(&reference),
            &evidence_summary,
            &missing_evidence,
        );

        assert_eq!(updated["templateReference"], reference);
        assert_eq!(updated["templateEvidenceSummary"], evidence_summary);
        assert_eq!(updated["missingEvidence"], missing_evidence);
        assert_eq!(
            updated["templateAdaptation"]["templateReferenceId"],
            json!("docs-page")
        );
        assert_eq!(updated["source"]["templateReference"], reference);
        assert_eq!(
            updated["source"]["templateEvidenceSummary"],
            evidence_summary
        );
        assert_eq!(updated["source"]["missingEvidence"], missing_evidence);
        assert_eq!(
            updated["source"]["templateAdaptation"]["templateReferenceId"],
            json!("docs-page")
        );
    }

    #[test]
    fn generated_template_reference_id_is_ignored_for_intent() {
        let source_refs = json!({
            "template_reference_id": "generated-static-page:550e8400-e29b-41d4-a716-446655440000"
        });

        assert!(
            static_page_template_reference_for_intent(&json!({}), &source_refs)
                .expect("generated reference should parse")
                .is_none()
        );

        let payload = json!({
            "templateReferenceId": "static-page-template:550e8400-e29b-41d4-a716-446655440000"
        });

        assert!(
            static_page_template_reference_payload_for_intent(&payload, &json!({}))
                .expect("generated reference payload should parse")
                .is_null()
        );
    }
}
