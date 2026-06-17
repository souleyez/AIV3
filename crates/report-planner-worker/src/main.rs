use anyhow::{anyhow, Result};
use chrono::Utc;
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use storage::{
    AssetItemRecord, AssetProfileRecord, NewReportPlanAstVersion, PgStorage,
    DEFAULT_LOCAL_DATABASE_URL,
};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "report";
const DEFAULT_TASK_KEY: &str = "plan_report_ast";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("report_planner_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("REPORT_PLANNER_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        std::env::var("REPORT_PLANNER_TASK_KEY").unwrap_or_else(|_| DEFAULT_TASK_KEY.to_string());
    let poll_interval = std::env::var("REPORT_PLANNER_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "REPORT_PLANNER_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let wake_subject = workflow_task_enqueued_subject(&queue, &task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("report_planner_worker.{queue}.{task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        %task_key,
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "report-planner-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, task).await
                {
                    tracing::error!(error = ?error, "report planner task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "report planner failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    let report_plan_id = execution
        .report_plan_id
        .ok_or_else(|| anyhow!("workflow execution {} has no report plan id", execution.id))?;
    let report_plan = storage
        .report_plans()
        .get_by_id(task.tenant_id, report_plan_id)
        .await?
        .ok_or_else(|| anyhow!("report plan {} not found", report_plan_id))?;

    let asset_profile_summary =
        match load_report_plan_asset_profile_summary(storage, &report_plan).await {
            Ok(summary) => summary,
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    report_plan_id = %report_plan.id,
                    dataset_id = %report_plan.dataset_id,
                    "report planner skipped asset profile summary after storage failure"
                );
                empty_report_asset_profile_summary()
            }
        };
    let ast = build_report_ast_with_asset_profile_summary(&report_plan, asset_profile_summary);

    let process_result: Result<()> = async {
        let ast_version = storage
            .report_plan_ast_versions()
            .create_next_version(
                task.tenant_id,
                report_plan.id,
                &NewReportPlanAstVersion {
                    ast: ast.clone(),
                    created_at: Utc::now(),
                },
            )
            .await?;
        let outcome = json!({
            "report_plan_id": report_plan.id,
            "planner": "report-planner-worker",
            "ast_version_id": ast_version.id,
            "ast_version_no": ast_version.version_no,
            "module_count": ast
                .get("modules")
                .and_then(Value::as_array)
                .map(|modules| modules.len())
                .unwrap_or(0),
        });

        storage
            .report_plans()
            .mark_planned(
                task.tenant_id,
                report_plan.id,
                ast_version.id,
                &ast,
                Utc::now(),
            )
            .await?;
        platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepCompleted {
                task_key: task.task_key.clone(),
                output: Some(outcome),
            },
        )
        .await?;
        Ok(())
    }
    .await;

    if let Err(error) = process_result {
        let error_message = error.to_string();
        if let Err(signal_error) = platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepFailed {
                task_key: task.task_key.clone(),
                error: error_message.clone(),
            },
        )
        .await
        {
            tracing::error!(
                error = ?signal_error,
                task_id = %task.id,
                "report planner failed to send workflow step_failed signal"
            );
        }

        storage
            .workflow_tasks()
            .mark_failed(task.id, &error_message, Utc::now())
            .await?;

        return Err(error);
    }

    storage
        .workflow_tasks()
        .mark_succeeded(task.id, Utc::now())
        .await?;

    tracing::info!(
        task_id = %task.id,
        execution_id = %task.execution_id,
        report_plan_id = %report_plan.id,
        "report planner task completed"
    );

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReportPlanFocus {
    XinbaiOperations,
    XinbaiTakeHighOpportunity,
    XinbaiRisk,
    GeneralDatasetReport,
}

impl ReportPlanFocus {
    fn as_str(self) -> &'static str {
        match self {
            Self::XinbaiOperations => "xinbai_operations",
            Self::XinbaiTakeHighOpportunity => "xinbai_take_high_opportunity",
            Self::XinbaiRisk => "xinbai_risk",
            Self::GeneralDatasetReport => "general_dataset_report",
        }
    }

    fn template_candidate(self) -> Option<&'static str> {
        match self {
            Self::XinbaiOperations | Self::XinbaiTakeHighOpportunity | Self::XinbaiRisk => {
                Some("xinbai-functional-modular-template-20260604")
            }
            Self::GeneralDatasetReport => None,
        }
    }

    fn is_xinbai(self) -> bool {
        self.template_candidate().is_some()
    }
}

#[cfg(test)]
fn build_report_ast(plan: &domain_model::ReportPlan) -> Value {
    build_report_ast_with_asset_profile_summary(plan, empty_report_asset_profile_summary())
}

fn build_report_ast_with_asset_profile_summary(
    plan: &domain_model::ReportPlan,
    asset_profile_summary: Value,
) -> Value {
    let focus = infer_report_plan_focus(plan);
    let modules = match focus {
        ReportPlanFocus::XinbaiOperations
        | ReportPlanFocus::XinbaiTakeHighOpportunity
        | ReportPlanFocus::XinbaiRisk => build_xinbai_modules(focus),
        ReportPlanFocus::GeneralDatasetReport => build_general_dataset_modules(),
    };
    let modules = apply_asset_profile_summary_to_report_modules(modules, &asset_profile_summary);

    json!({
        "schema_version": "0.2.0",
        "planner": "report-planner-worker",
        "planner_mode": "deterministic_business_template",
        "plan_id": plan.id,
        "tenant_id": plan.tenant_id,
        "dataset_id": plan.dataset_id,
        "title": plan.title,
        "objective": plan.objective,
        "theme_key": plan.theme_key,
        "focus_key": focus.as_str(),
        "template_candidate": focus.template_candidate(),
        "asset_profile_summary": asset_profile_summary,
        "layout_policy": {
            "default_time_grain": if focus.is_xinbai() { "month" } else { "source_scope" },
            "mobile_first": focus.is_xinbai(),
            "module_priority": "focus_then_standard_operations",
            "data_refresh": "bind_to_current_dataset_snapshot"
        },
        "quality_gates": [
            {
                "code": "scope_bound_to_dataset",
                "severity": "required",
                "rule": "Only use data authorized for the report dataset and current workflow."
            },
            {
                "code": "no_raw_secret_output",
                "severity": "required",
                "rule": "Do not expose credentials, connection strings, or private source URLs in rendered output."
            },
            {
                "code": "export_manifest_required",
                "severity": "warning",
                "rule": "When a rendered report is published, expose table-data.csv, report.ppt, and report.md when available."
            }
        ],
        "modules": modules
    })
}

async fn load_report_plan_asset_profile_summary(
    storage: &PgStorage,
    plan: &domain_model::ReportPlan,
) -> Result<Value> {
    const ASSET_LIMIT: usize = 24;

    let assets = storage
        .asset_items()
        .list_by_dataset_ids(plan.tenant_id, &[plan.dataset_id], ASSET_LIMIT)
        .await?;
    if assets.is_empty() {
        return Ok(empty_report_asset_profile_summary());
    }

    let mut hints = Vec::new();
    for asset in assets {
        let profiles = storage
            .asset_items()
            .list_profiles(plan.tenant_id, asset.id)
            .await?;
        for profile in profiles {
            if let Some(hint) = report_asset_profile_hint(&asset, &profile) {
                hints.push(hint);
            }
        }
    }

    Ok(build_report_asset_profile_summary(hints))
}

fn apply_asset_profile_summary_to_report_modules(
    mut modules: Vec<Value>,
    asset_profile_summary: &Value,
) -> Vec<Value> {
    if !asset_profile_summary_has_hints(asset_profile_summary) {
        return modules;
    }

    let module = json!({
        "kind": "asset_materials",
        "title": "资产素材与主题线索",
        "binding_slot": "assets.profile_summary",
        "purpose": "根据当前数据集的图片、视频、PPT、文档资产画像，为报告选择可用素材、视觉主题和需进一步核验的证据入口。",
        "asset_profile_summary": asset_profile_summary,
        "evidence_policy": "profile_hints_are_planning_signals_not_citations"
    });

    let insert_at = modules
        .iter()
        .position(|item| item.get("kind").and_then(Value::as_str) == Some("scope_summary"))
        .map(|index| index + 1)
        .or_else(|| {
            modules
                .iter()
                .position(|item| item.get("kind").and_then(Value::as_str) == Some("global_filters"))
                .map(|index| index + 1)
        })
        .unwrap_or(0);
    modules.insert(insert_at, module);
    modules
}

fn asset_profile_summary_has_hints(summary: &Value) -> bool {
    summary
        .get("count")
        .and_then(Value::as_u64)
        .is_some_and(|count| count > 0)
}

fn empty_report_asset_profile_summary() -> Value {
    json!({
        "count": 0,
        "hints": [],
        "primary_terms": [],
        "kind_counts": {},
        "profile_kind_counts": {},
        "planning_policy": [
            "asset profiles are optional planning signals",
            "do not cite asset profile hints as exact source evidence"
        ]
    })
}

fn report_asset_profile_hint(
    asset: &AssetItemRecord,
    profile: &AssetProfileRecord,
) -> Option<Value> {
    if !profile.attributes.is_object() {
        return None;
    }
    let summary = report_asset_profile_summary_text(&profile.attributes)
        .unwrap_or_else(|| compact_report_text(&asset.title, 240));
    if summary.is_empty() {
        return None;
    }
    Some(json!({
        "asset_id": asset.id,
        "title": compact_report_text(&asset.title, 160),
        "asset_kind": compact_report_text(&asset.asset_kind, 80),
        "source_kind": compact_report_text(&asset.source_kind, 80),
        "profile_kind": compact_report_text(&profile.profile_kind, 80),
        "summary": summary,
        "noun_terms": report_asset_profile_terms(&profile.attributes),
        "facets": report_asset_profile_facets(&profile.attributes),
    }))
}

fn build_report_asset_profile_summary(hints: Vec<Value>) -> Value {
    const HINT_LIMIT: usize = 8;
    const TERM_LIMIT: usize = 16;

    let mut compact_hints = Vec::new();
    let mut primary_terms = Vec::new();
    let mut seen_terms = BTreeSet::new();
    let mut kind_counts = BTreeMap::<String, usize>::new();
    let mut profile_kind_counts = BTreeMap::<String, usize>::new();

    for hint in &hints {
        let asset_kind = hint
            .get("asset_kind")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        *kind_counts.entry(asset_kind).or_insert(0) += 1;

        let profile_kind = hint
            .get("profile_kind")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        *profile_kind_counts.entry(profile_kind).or_insert(0) += 1;

        if let Some(terms) = hint.get("noun_terms").and_then(Value::as_array) {
            for term in terms.iter().filter_map(Value::as_str) {
                if seen_terms.insert(term.to_string()) && primary_terms.len() < TERM_LIMIT {
                    primary_terms.push(term.to_string());
                }
            }
        }

        if compact_hints.len() < HINT_LIMIT {
            compact_hints.push(json!({
                "asset_id": hint.get("asset_id").cloned().unwrap_or(Value::Null),
                "title": hint.get("title").cloned().unwrap_or(Value::Null),
                "asset_kind": hint.get("asset_kind").cloned().unwrap_or(Value::Null),
                "source_kind": hint.get("source_kind").cloned().unwrap_or(Value::Null),
                "profile_kind": hint.get("profile_kind").cloned().unwrap_or(Value::Null),
                "summary": hint.get("summary").cloned().unwrap_or(Value::Null),
                "noun_terms": hint.get("noun_terms").cloned().unwrap_or_else(|| json!([])),
                "facets": hint.get("facets").cloned().unwrap_or_else(|| json!([])),
            }));
        }
    }

    json!({
        "count": hints.len(),
        "hints": compact_hints,
        "primary_terms": primary_terms,
        "kind_counts": kind_counts,
        "profile_kind_counts": profile_kind_counts,
        "planning_policy": [
            "use asset profiles to choose report material sections, visual themes, and follow-up evidence reads",
            "asset profiles are compact planning signals and are not citable exact source evidence",
            "exact numbers, quotations, source wording, and media timestamps still require retrieval evidence, source detail, database rows, or media context"
        ]
    })
}

fn report_asset_profile_summary_text(attributes: &Value) -> Option<String> {
    [
        "summary",
        "visual_summary",
        "visualSummary",
        "description",
        "caption",
        "ocr_text",
        "ocrText",
        "transcript_summary",
        "transcriptSummary",
    ]
    .iter()
    .find_map(|key| attributes.get(*key).and_then(Value::as_str))
    .map(|value| compact_report_text(value, 240))
    .filter(|value| !value.is_empty())
}

fn report_asset_profile_terms(attributes: &Value) -> Vec<String> {
    const TERM_LIMIT: usize = 16;
    let mut terms = BTreeSet::new();
    for key in [
        "noun_terms",
        "nounTermHints",
        "tags",
        "topicTags",
        "keywords",
        "entities",
        "field_candidates",
        "fieldCandidates",
        "table_like_signals",
        "tableLikeSignals",
        "outline",
        "scene_summaries",
        "sceneSummaries",
    ] {
        collect_report_asset_profile_terms(attributes.get(key), &mut terms, TERM_LIMIT);
        if terms.len() >= TERM_LIMIT {
            break;
        }
    }
    terms.into_iter().take(TERM_LIMIT).collect()
}

fn collect_report_asset_profile_terms(
    value: Option<&Value>,
    terms: &mut BTreeSet<String>,
    limit: usize,
) {
    if terms.len() >= limit {
        return;
    }
    match value {
        Some(Value::String(text)) => {
            let text = compact_report_text(text, 80);
            if !text.is_empty() {
                terms.insert(text);
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                collect_report_asset_profile_terms(Some(item), terms, limit);
                if terms.len() >= limit {
                    break;
                }
            }
        }
        Some(Value::Object(object)) => {
            for key in ["name", "label", "text", "title", "field", "type", "value"] {
                if let Some(text) = object.get(key).and_then(Value::as_str) {
                    let text = compact_report_text(text, 80);
                    if !text.is_empty() {
                        terms.insert(text);
                    }
                }
                if terms.len() >= limit {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn report_asset_profile_facets(attributes: &Value) -> Vec<String> {
    let mut facets = Vec::new();
    for (label, key) in [
        ("文档类型", "document_kind"),
        ("版式", "layout_type"),
        ("风险", "risk_level"),
        ("状态", "media_parse_status"),
        ("幻灯片数", "slide_count_estimate"),
        ("字幕段数", "transcript_segment_count"),
        ("场景数", "scene_count"),
        ("关键帧 OCR", "keyframe_ocr_count"),
    ] {
        if let Some(text) = report_profile_scalar_text(attributes.get(key)) {
            facets.push(format!("{label}: {text}"));
        }
        if facets.len() >= 8 {
            break;
        }
    }
    facets
}

fn report_profile_scalar_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(compact_report_text(text, 80)),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        _ => None,
    }
    .filter(|text| !text.is_empty())
}

fn compact_report_text(value: &str, max_chars: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

fn infer_report_plan_focus(plan: &domain_model::ReportPlan) -> ReportPlanFocus {
    let haystack = format!("{} {} {}", plan.title, plan.objective, plan.theme_key).to_lowercase();

    if contains_any(
        &haystack,
        &[
            "取高",
            "销售缺口",
            "助推",
            "高分成",
            "提成",
            "take high",
            "commission",
        ],
    ) {
        return ReportPlanFocus::XinbaiTakeHighOpportunity;
    }

    if contains_any(
        &haystack,
        &[
            "低活跃",
            "风险",
            "租售比",
            "客流下降",
            "无销售",
            "risk",
            "inactive",
        ],
    ) {
        return ReportPlanFocus::XinbaiRisk;
    }

    if contains_any(
        &haystack,
        &[
            "新百",
            "新世界",
            "经营",
            "月报",
            "门店",
            "品牌",
            "收入",
            "销售",
            "健康度",
            "retail",
            "operation",
        ],
    ) {
        return ReportPlanFocus::XinbaiOperations;
    }

    ReportPlanFocus::GeneralDatasetReport
}

fn contains_any(haystack: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| haystack.contains(keyword))
}

fn build_xinbai_modules(focus: ReportPlanFocus) -> Vec<Value> {
    let modules = vec![
        json!({
            "kind": "global_filters",
            "title": "筛选条件",
            "binding_slot": "filters.global",
            "purpose": "统一控制时间、分区、门店、品类与经营模式。",
            "defaults": {
                "time_range": "latest_month",
                "business_mode": "all"
            },
            "controls": [
                {"key": "time_range", "type": "month_or_range", "label": "时间"},
                {"key": "region_or_store", "type": "compact_select", "label": "区域/门店"},
                {"key": "category", "type": "select", "label": "品类"},
                {"key": "business_mode", "type": "segmented", "label": "经营模式", "options": ["全部", "租赁", "联营", "自营"]}
            ]
        }),
        json!({
            "kind": "operating_overview",
            "title": "经营总览",
            "binding_slot": "operations.overview",
            "purpose": "呈现收入、同比、取高达成、低活跃和新增风险的核心经营状态。",
            "metrics": [
                "total_revenue",
                "revenue_yoy",
                "last_month_take_high_store_count",
                "current_month_expected_take_high_store_count",
                "low_activity_store_count",
                "new_low_activity_store_count"
            ]
        }),
        json!({
            "kind": "monthly_sales_trend",
            "title": "月度销售趋势",
            "binding_slot": "operations.sales_trend",
            "purpose": "在同一张图展示总览、区域与门店销售趋势，并保持筛选口径一致。",
            "chart": {
                "type": "line",
                "series_policy": "total_then_selected_region_or_store"
            }
        }),
        json!({
            "kind": "opportunity_category_share",
            "title": "机会品类占比",
            "binding_slot": "take_high.category_share",
            "purpose": "展示取高中高机会店铺在当前筛选范围内的品类分布。",
            "chart": {
                "type": "pie",
                "label_policy": "show_full_category_name_below_chart"
            }
        }),
        json!({
            "kind": "risk_category_share",
            "title": "风险品类占比",
            "binding_slot": "risk.category_share",
            "purpose": "展示风险店铺在当前筛选范围内的品类分布。",
            "chart": {
                "type": "pie",
                "label_policy": "show_full_category_name_below_chart"
            }
        }),
        json!({
            "kind": "operating_health_score",
            "title": "经营健康度评分",
            "binding_slot": "health.score_by_region_store",
            "purpose": "按分区展示评分，支持展开到分店，并显示收入同比、客流同比和平均租售比。",
            "ranking": {
                "default_level": "region",
                "primary_metric": "revenue_yoy_score",
                "expandable_to": "store"
            },
            "fields": [
                "region_or_store",
                "revenue",
                "revenue_yoy",
                "traffic_yoy",
                "traffic_mom",
                "average_rent_sales_ratio",
                "expected_take_high_store_count",
                "expected_take_high_incremental_rent",
                "risk_store_count",
                "new_risk_store_count"
            ]
        }),
        json!({
            "kind": "take_high_line_stores",
            "title": "取高线附近门店",
            "binding_slot": "take_high.line_distance_stores",
            "purpose": "合并未达线和已达线门店，按距离取高线绝对值由近到远排序。",
            "chart": {
                "type": "paired_bar",
                "rank_by": "absolute_distance_to_take_high_line_asc",
                "below_line_color": "blue",
                "above_line_color": "green"
            },
            "fields": [
                "store",
                "brand",
                "current_revenue",
                "forecast_revenue",
                "take_high_line_revenue",
                "distance_to_take_high_line",
                "needs_push"
            ]
        }),
        json!({
            "kind": "risk_warning",
            "title": "风险提示",
            "binding_slot": "risk.warning",
            "purpose": "列出持续低活跃、最新低活跃、租售比风险和客流降低预警。",
            "sections": [
                "persistent_low_activity_brands",
                "new_low_activity_brands_excluding_persistent",
                "rent_sales_ratio_distribution",
                "traffic_decline_warning"
            ]
        }),
        json!({
            "kind": "export_manifest",
            "title": "导出文件",
            "binding_slot": "exports.files",
            "purpose": "发布后提供表格数据、PPT 和 Markdown 文本下载。",
            "files": [
                "table-data.csv",
                "report.ppt",
                "report.md"
            ]
        }),
    ];

    prioritize_modules(modules, xinbai_priority(focus))
}

fn xinbai_priority(focus: ReportPlanFocus) -> &'static [&'static str] {
    match focus {
        ReportPlanFocus::XinbaiTakeHighOpportunity => &[
            "take_high_line_stores",
            "opportunity_category_share",
            "operating_overview",
            "monthly_sales_trend",
        ],
        ReportPlanFocus::XinbaiRisk => &[
            "risk_warning",
            "risk_category_share",
            "operating_health_score",
            "operating_overview",
        ],
        ReportPlanFocus::XinbaiOperations => &[
            "operating_overview",
            "monthly_sales_trend",
            "opportunity_category_share",
            "risk_category_share",
            "operating_health_score",
        ],
        ReportPlanFocus::GeneralDatasetReport => &[],
    }
}

fn prioritize_modules(mut modules: Vec<Value>, priority: &[&str]) -> Vec<Value> {
    let mut ordered = Vec::with_capacity(modules.len());

    if let Some(module) = take_module_by_kind(&mut modules, "global_filters") {
        ordered.push(module);
    }

    for kind in priority {
        if let Some(module) = take_module_by_kind(&mut modules, kind) {
            ordered.push(module);
        }
    }

    ordered.extend(modules);
    ordered
}

fn take_module_by_kind(modules: &mut Vec<Value>, kind: &str) -> Option<Value> {
    modules
        .iter()
        .position(|module| module.get("kind").and_then(Value::as_str) == Some(kind))
        .map(|index| modules.remove(index))
}

fn build_general_dataset_modules() -> Vec<Value> {
    vec![
        json!({
            "kind": "scope_summary",
            "title": "资料范围",
            "binding_slot": "dataset.scope_summary",
            "purpose": "说明本次报告使用的数据集、文档范围和时间口径。"
        }),
        json!({
            "kind": "key_findings",
            "title": "关键结论",
            "binding_slot": "analysis.key_findings",
            "purpose": "汇总对用户目标最相关的结论，并保留证据引用。"
        }),
        json!({
            "kind": "data_quality",
            "title": "数据质量",
            "binding_slot": "dataset.quality",
            "purpose": "列出缺失字段、异常日期、重复资料和需要人工确认的口径。"
        }),
        json!({
            "kind": "evidence_table",
            "title": "证据明细",
            "binding_slot": "evidence.table",
            "purpose": "按来源、字段、页码或记录定位展示支撑材料。"
        }),
        json!({
            "kind": "recommended_actions",
            "title": "建议动作",
            "binding_slot": "analysis.actions",
            "purpose": "给出下一步处理建议，区分可自动执行和需人工确认事项。"
        }),
        json!({
            "kind": "export_manifest",
            "title": "导出文件",
            "binding_slot": "exports.files",
            "purpose": "发布后提供可下载的报告附件。"
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, ReportPlan, ReportPlanId, ReportPlanStatus, TenantId};

    fn sample_plan(title: &str, objective: &str, theme_key: &str) -> ReportPlan {
        ReportPlan {
            id: ReportPlanId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: title.to_string(),
            objective: objective.to_string(),
            status: ReportPlanStatus::Draft,
            theme_key: theme_key.to_string(),
            current_ast_version_id: None,
            modules: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn module_kinds(ast: &Value) -> Vec<&str> {
        ast.get("modules")
            .and_then(Value::as_array)
            .expect("ast should expose modules")
            .iter()
            .map(|module| {
                module
                    .get("kind")
                    .and_then(Value::as_str)
                    .expect("module should expose kind")
            })
            .collect()
    }

    #[test]
    fn take_high_plan_uses_xinbai_template_and_frontloads_take_high() {
        let plan = sample_plan(
            "最近可取高门店机会榜",
            "按销售缺口和需助推门店生成经营报表",
            "default-dark",
        );

        let ast = build_report_ast(&plan);
        let kinds = module_kinds(&ast);

        assert_eq!(ast["schema_version"], "0.2.0");
        assert_eq!(ast["focus_key"], "xinbai_take_high_opportunity");
        assert_eq!(
            ast["template_candidate"],
            "xinbai-functional-modular-template-20260604"
        );
        assert_eq!(kinds[0], "global_filters");
        assert_eq!(kinds[1], "take_high_line_stores");
        assert!(kinds.contains(&"opportunity_category_share"));
        assert!(kinds.contains(&"export_manifest"));
    }

    #[test]
    fn risk_plan_frontloads_risk_modules() {
        let plan = sample_plan(
            "低活跃品牌报表",
            "识别租售比风险和客流下降门店",
            "xinbai-mobile",
        );

        let ast = build_report_ast(&plan);
        let kinds = module_kinds(&ast);

        assert_eq!(ast["focus_key"], "xinbai_risk");
        assert_eq!(kinds[0], "global_filters");
        assert_eq!(kinds[1], "risk_warning");
        assert_eq!(kinds[2], "risk_category_share");
    }

    #[test]
    fn operations_plan_uses_monthly_default_and_operations_order() {
        let plan = sample_plan("新世界百货经营月报", "查看整体经营状况和健康度", "default");

        let ast = build_report_ast(&plan);
        let kinds = module_kinds(&ast);

        assert_eq!(ast["focus_key"], "xinbai_operations");
        assert_eq!(ast["layout_policy"]["default_time_grain"], "month");
        assert_eq!(kinds[0], "global_filters");
        assert_eq!(kinds[1], "operating_overview");
        assert_eq!(kinds[2], "monthly_sales_trend");
    }

    #[test]
    fn general_plan_has_dataset_modules_without_skeleton_language() {
        let plan = sample_plan(
            "合同资料分析",
            "整理合同条款、异常字段和建议动作",
            "default",
        );

        let ast = build_report_ast(&plan);
        let serialized = serde_json::to_string(&ast).expect("ast should serialize");
        let kinds = module_kinds(&ast);

        assert_eq!(ast["focus_key"], "general_dataset_report");
        assert!(ast["template_candidate"].is_null());
        assert_eq!(kinds[0], "scope_summary");
        assert!(kinds.contains(&"evidence_table"));
        assert!(!serialized.to_lowercase().contains("placeholder"));
        assert!(!serialized.to_lowercase().contains("skeleton"));
    }

    #[test]
    fn report_ast_includes_asset_profile_summary_module_when_available() {
        let plan = sample_plan("服装图库分析", "按图库主题生成商品报告", "default");
        let asset_summary = json!({
            "count": 2,
            "hints": [{
                "asset_id": "asset-1",
                "title": "蓝色连衣裙",
                "asset_kind": "image",
                "profile_kind": "image_semantic",
                "summary": "蓝色夏季连衣裙主视觉",
                "noun_terms": ["连衣裙", "蓝色"],
                "facets": ["文档类型: image"]
            }],
            "primary_terms": ["连衣裙", "蓝色"],
            "kind_counts": {"image": 1, "presentation": 1},
            "profile_kind_counts": {"image_semantic": 1, "presentation_outline": 1}
        });

        let ast = build_report_ast_with_asset_profile_summary(&plan, asset_summary);
        let kinds = module_kinds(&ast);

        assert_eq!(ast["asset_profile_summary"]["count"], json!(2));
        assert_eq!(kinds[0], "scope_summary");
        assert_eq!(kinds[1], "asset_materials");
        assert_eq!(
            ast["modules"][1]["binding_slot"],
            json!("assets.profile_summary")
        );
        assert_eq!(
            ast["modules"][1]["evidence_policy"],
            json!("profile_hints_are_planning_signals_not_citations")
        );
    }

    #[test]
    fn report_asset_profile_summary_keeps_compact_safe_fields() {
        let summary = build_report_asset_profile_summary(vec![json!({
            "asset_id": "asset-raw",
            "title": "发布会视频",
            "asset_kind": "video",
            "source_kind": "document",
            "profile_kind": "video_summary",
            "summary": "视频包含门店陈列讲解。",
            "noun_terms": ["门店", "陈列"],
            "facets": ["场景数: 3"],
            "raw_provider_payload": {"object_key": "must-not-leak"}
        })]);

        assert_eq!(summary["count"], json!(1));
        assert_eq!(summary["kind_counts"]["video"], json!(1));
        assert_eq!(summary["profile_kind_counts"]["video_summary"], json!(1));
        assert_eq!(
            summary["hints"][0]["summary"],
            json!("视频包含门店陈列讲解。")
        );
        assert!(summary["hints"][0].get("raw_provider_payload").is_none());
        assert!(summary["planning_policy"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item
                == "asset profiles are compact planning signals and are not citable exact source evidence")));
    }
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "report planner received task wake signal");
    }
}
