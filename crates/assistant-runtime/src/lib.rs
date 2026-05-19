use contracts::{
    AssistantRunCodexActionContractView, AssistantRunCodexContextBudgetView,
    AssistantRunCodexContextPackageView, AssistantRunExecutorTransportView,
};
use domain_model::{Dataset, DatasetId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

const MEDIA_HINTS: &[&str] = &[
    "音视频",
    "音频",
    "视频",
    "录音",
    "转写",
    "字幕",
    "会议",
    "访谈",
    "关键帧",
    "ocr",
    "OCR",
];

const VIDEO_PPT_SOURCE_HINTS: &[&str] = &[
    "视频",
    "mp4",
    "mov",
    "m4v",
    "webm",
    "公开视频",
    "视频地址",
    "视频链接",
    "url",
    "URL",
    "上传",
];

const VIDEO_PPT_OUTPUT_HINTS: &[&str] = &[
    "ppt",
    "PPT",
    "powerpoint",
    "PowerPoint",
    "幻灯片",
    "课件",
    "原文",
    "字幕",
    "转写",
    "讲稿",
    "提取",
];

const BUSINESS_HINTS: &[(&str, &[&str])] = &[
    (
        "订单",
        &[
            "订单", "销售", "营收", "收入", "库存", "发货", "客单", "转化", "复购", "经营",
        ],
    ),
    (
        "客服",
        &[
            "客服", "工单", "投诉", "满意", "售后", "咨询", "回复", "评价",
        ],
    ),
    (
        "企业问答",
        &[
            "企业问答",
            "制度",
            "流程",
            "员工",
            "手册",
            "政策",
            "组织",
            "公司介绍",
            "FAQ",
            "问答",
        ],
    ),
    (
        "网页采集",
        &[
            "网页", "采集", "官网", "竞品", "新闻", "页面", "站点", "爬取", "抓取",
        ],
    ),
    ("音视频", MEDIA_HINTS),
    ("录音", MEDIA_HINTS),
    ("视频", MEDIA_HINTS),
    ("会议", MEDIA_HINTS),
];

const CONVERSATION_HINTS: &[&str] = &[
    "刚才",
    "上面",
    "之前",
    "继续",
    "按你说的",
    "这个",
    "那版",
    "上一版",
    "草稿",
    "修改",
    "调整",
    "确认",
    "不要",
    "改成",
    "换成",
];

const STATIC_PAGE_HINTS: &[&str] = &[
    "静态页",
    "静态页面",
    "页面规划",
    "一页",
    "生成页面",
    "落地页",
    "模块",
    "效果图",
    "出图",
];

const REPORT_HINTS: &[&str] = &[
    "报表",
    "报告",
    "周报",
    "月报",
    "经营分析",
    "汇报",
    "可视化",
    "看板",
    "dashboard",
];

const DATA_QUESTION_HINTS: &[&str] = &[
    "分析",
    "总结",
    "趋势",
    "原因",
    "风险",
    "机会",
    "对比",
    "明细",
    "指标",
    "数据",
    "检索",
    "查找",
    "引用",
    "怎么操作",
    "申请",
    "审批",
    "入口",
    "音视频",
    "音频",
    "视频",
    "录音",
    "转写",
    "字幕",
    "会议",
    "访谈",
    "关键帧",
    "ocr",
    "OCR",
];

const PLAIN_GENERATION_HINTS: &[&str] = &[
    "写一句",
    "写一段",
    "写个",
    "写一个",
    "帮我写",
    "润色",
    "改写",
    "翻译",
    "起名",
    "欢迎语",
    "文案",
    "话术",
    "怎么说",
    "表达",
];

const EXPLICIT_DATA_NEED_HINTS: &[&str] = &[
    "基于",
    "根据",
    "结合",
    "引用",
    "检索",
    "查找",
    "资料",
    "数据",
    "文档",
    "知识库",
    "证据",
    "来源",
    "原文",
    "明细",
    "指标",
    "趋势",
    "风险",
    "分析",
    "总结",
    "对比",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeCandidateType {
    Dataset,
    ConversationMemory,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeConfidence {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopeCandidate {
    #[serde(rename = "type")]
    pub candidate_type: ScopeCandidateType,
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub visibility: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lifecycle: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub document_count: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub estimated_word_count: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parse_status_summary: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub latest_activity: String,
    pub confidence: ScopeConfidence,
    pub reason: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_hints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub noun_term_hints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub section_title_hints: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScopePlan {
    pub candidates: Vec<ScopeCandidate>,
    pub selected_scope: Value,
    pub hint: String,
    pub intent: String,
}

#[derive(Clone, Debug)]
pub struct ScopePlannerInput<'a> {
    pub prompt: &'a str,
    pub visible_datasets: &'a [Dataset],
    pub selected_dataset_id: Option<DatasetId>,
    pub conversation_memory_available: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexConversationExecutorStatus {
    DirectPassthrough,
    ShadowDryRun,
    PlanOnly,
    RejectedUnsafeContext,
    UnsupportedTransport,
}

impl CodexConversationExecutorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DirectPassthrough => "direct_passthrough",
            Self::ShadowDryRun => "shadow_dry_run",
            Self::PlanOnly => "plan_only",
            Self::RejectedUnsafeContext => "rejected_unsafe_context",
            Self::UnsupportedTransport => "unsupported_transport",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CodexConversationExecutorOutput {
    pub transport: AssistantRunExecutorTransportView,
    pub status: CodexConversationExecutorStatus,
    pub codex_invoked: bool,
    pub fallback_to_direct: bool,
    pub assistant_message: Option<String>,
    pub suggested_action: Option<Value>,
    pub planned_action_types: Vec<String>,
    pub execution_trail: Vec<Value>,
    pub context_budget: AssistantRunCodexContextBudgetView,
    pub model_gateway: Value,
    pub output_schema: Option<Value>,
    pub host_invocation: Option<Value>,
}

pub fn execute_codex_conversation_plan(
    package: &AssistantRunCodexContextPackageView,
) -> CodexConversationExecutorOutput {
    if let Some(reason) = unsafe_codex_context_reason(package) {
        return CodexConversationExecutorOutput {
            transport: package.executor_transport.clone(),
            status: CodexConversationExecutorStatus::RejectedUnsafeContext,
            codex_invoked: false,
            fallback_to_direct: true,
            assistant_message: None,
            suggested_action: None,
            planned_action_types: Vec::new(),
            execution_trail: vec![json!({
                "kind": "codex_executor.rejected",
                "reason": reason,
                "fallback": "direct",
                "supply_quality": codex_executor_supply_quality_summary(package),
                "model_gateway": codex_executor_model_gateway_summary(package),
            })],
            context_budget: package.context_budget.clone(),
            model_gateway: codex_executor_model_gateway_summary(package),
            output_schema: None,
            host_invocation: None,
        };
    }

    match package.executor_transport {
        AssistantRunExecutorTransportView::Direct => CodexConversationExecutorOutput {
            transport: package.executor_transport.clone(),
            status: CodexConversationExecutorStatus::DirectPassthrough,
            codex_invoked: false,
            fallback_to_direct: true,
            assistant_message: None,
            suggested_action: None,
            planned_action_types: Vec::new(),
            execution_trail: vec![json!({
                "kind": "codex_executor.direct_passthrough",
                "message": "direct executor remains active",
                "supply_quality": codex_executor_supply_quality_summary(package),
                "model_gateway": codex_executor_model_gateway_summary(package),
            })],
            context_budget: package.context_budget.clone(),
            model_gateway: codex_executor_model_gateway_summary(package),
            output_schema: None,
            host_invocation: None,
        },
        AssistantRunExecutorTransportView::CodexDryRun => CodexConversationExecutorOutput {
            transport: package.executor_transport.clone(),
            status: CodexConversationExecutorStatus::ShadowDryRun,
            codex_invoked: false,
            fallback_to_direct: true,
            assistant_message: None,
            suggested_action: None,
            planned_action_types: package.action_types(),
            execution_trail: vec![json!({
                "kind": "codex_executor.shadow_dry_run",
                "assistant_run_id": package.assistant_run_id.to_string(),
                "available_action_count": package.available_actions.len(),
                "supply_quality": codex_executor_supply_quality_summary(package),
                "model_gateway": codex_executor_model_gateway_summary(package),
                "message": "Codex executor not invoked; direct flow remains authoritative",
            })],
            context_budget: package.context_budget.clone(),
            model_gateway: codex_executor_model_gateway_summary(package),
            output_schema: Some(codex_executor_action_output_schema(package)),
            host_invocation: None,
        },
        AssistantRunExecutorTransportView::CodexPlanOnly => {
            let suggested_action = codex_executor_suggest_action(package);
            let output_schema = codex_executor_action_output_schema(package);
            CodexConversationExecutorOutput {
                transport: package.executor_transport.clone(),
                status: CodexConversationExecutorStatus::PlanOnly,
                codex_invoked: false,
                fallback_to_direct: true,
                assistant_message: None,
                suggested_action: suggested_action.clone(),
                planned_action_types: package.action_types(),
                execution_trail: vec![json!({
                    "kind": "codex_executor.plan_only",
                    "assistant_run_id": package.assistant_run_id.to_string(),
                    "transport": package.executor_transport.as_str(),
                    "planned_action_types": package.action_types(),
                    "suggested_action": suggested_action,
                    "context_budget": {
                        "quality_first": package.context_budget.quality_first,
                        "estimated_prompt_chars": package.context_budget.estimated_prompt_chars,
                        "budget_pressure": package.context_budget.budget_pressure,
                        "evidence_item_count": package.context_budget.evidence_item_count,
                        "selected_dataset_count": package.context_budget.selected_dataset_count,
                        "hidden_memory_item_count": package.context_budget.hidden_memory_item_count,
                        "trimmed_item_count": package.context_budget.trimmed_item_count,
                        "budget_item_count": package.context_budget.items.len(),
                    },
                    "output_schema": output_schema.clone(),
                    "supply_quality": codex_executor_supply_quality_summary(package),
                    "model_gateway": codex_executor_model_gateway_summary(package),
                    "tool_output_policy": package.tool_output_policy,
                })],
                context_budget: package.context_budget.clone(),
                model_gateway: codex_executor_model_gateway_summary(package),
                output_schema: Some(output_schema),
                host_invocation: None,
            }
        }
        AssistantRunExecutorTransportView::CodexExecSchema
        | AssistantRunExecutorTransportView::CodexSdkThread
        | AssistantRunExecutorTransportView::CodexAppServer
        | AssistantRunExecutorTransportView::CodexMcpServer => {
            let output_schema = codex_executor_action_output_schema(package);
            let host_invocation = codex_executor_host_invocation_blueprint(package, &output_schema);
            CodexConversationExecutorOutput {
                transport: package.executor_transport.clone(),
                status: CodexConversationExecutorStatus::UnsupportedTransport,
                codex_invoked: false,
                fallback_to_direct: true,
                assistant_message: None,
                suggested_action: None,
                planned_action_types: package.action_types(),
                execution_trail: vec![json!({
                    "kind": "codex_executor.unsupported_transport",
                    "transport": package.executor_transport.as_str(),
                    "fallback": "direct",
                    "supply_quality": codex_executor_supply_quality_summary(package),
                    "model_gateway": codex_executor_model_gateway_summary(package),
                    "host_invocation": host_invocation.clone(),
                    "message": "real Codex execution is not wired in assistant-runtime",
                })],
                context_budget: package.context_budget.clone(),
                model_gateway: codex_executor_model_gateway_summary(package),
                output_schema: Some(output_schema),
                host_invocation: Some(host_invocation),
            }
        }
    }
}

fn codex_executor_supply_quality_summary(package: &AssistantRunCodexContextPackageView) -> Value {
    let source = if package.supply_quality.is_null() {
        package
            .evidence_state
            .get("supply_quality")
            .or_else(|| package.evidence_state.get("supplyQuality"))
            .unwrap_or(&Value::Null)
    } else {
        &package.supply_quality
    };
    if source.is_null() {
        return json!({
            "status": "unknown",
            "suppliedItemCount": package.context_budget.evidence_item_count,
            "selectedDatasetCount": package.context_budget.selected_dataset_count,
            "hiddenMemoryItemCount": package.context_budget.hidden_memory_item_count,
        });
    }
    json!({
        "status": source.get("status").and_then(Value::as_str).unwrap_or("unknown"),
        "suppliedItemCount": source
            .get("suppliedItemCount")
            .or_else(|| source.get("supplied_item_count"))
            .and_then(Value::as_u64)
            .unwrap_or(package.context_budget.evidence_item_count as u64),
        "citationLocatorCount": source
            .get("citationLocatorCount")
            .or_else(|| source.get("citation_locator_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "fallbackChunkCount": source
            .get("fallbackChunkCount")
            .or_else(|| source.get("fallback_chunk_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "mediaContextCount": source
            .get("mediaContextCount")
            .or_else(|| source.get("media_context_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

fn codex_executor_model_gateway_summary(package: &AssistantRunCodexContextPackageView) -> Value {
    if package.model_gateway.is_null() {
        let capability_manifest = codex_executor_empty_capability_manifest();
        return json!({
            "lane": "unknown",
            "selected_model": Value::Null,
            "profile_available": false,
            "profile_id": Value::Null,
            "capabilities": [],
            "capability_manifest": capability_manifest,
            "codex_surface": {
                "wire_api": "unknown",
                "codex_compatible": false,
                "json_actions_supported": false,
                "tool_calls_supported": false,
                "real_execution_allowed_on_this_host": false,
                "real_execution_block_reason": "model_gateway_unknown",
            },
            "codex_real_execution_allowed": false,
        });
    }
    let selected_model = package
        .model_gateway
        .get("selected_model")
        .cloned()
        .unwrap_or(Value::Null);
    let profile = package.model_gateway.get("profile").unwrap_or(&Value::Null);
    let profile_id = profile.get("profile_id").cloned().unwrap_or(Value::Null);
    let capabilities = codex_executor_profile_capabilities(profile);
    let capability_manifest =
        codex_executor_capability_manifest_from_profile(profile, &capabilities);
    let wire_api = package
        .model_gateway
        .pointer("/profile/wire_api")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let auth_configured = package
        .model_gateway
        .pointer("/profile/auth/configured")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let real_execution_allowed_on_this_host = package
        .model_gateway
        .pointer("/safety/codex_real_execution_allowed_on_this_host")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let codex_compatible =
        codex_executor_capability_enabled(&capability_manifest, "codex_compatible")
            || matches!(
                wire_api,
                "codex_compatible_shim" | "codex-compatible-shim" | "codex_shim" | "provider_shim"
            );
    let json_actions_supported =
        codex_executor_capability_enabled(&capability_manifest, "json_mode")
            || matches!(wire_api, "responses" | "codex_compatible_shim");
    let tool_calls_supported =
        codex_executor_capability_enabled(&capability_manifest, "tool_calling");
    let real_execution_block_reason = codex_executor_real_execution_block_reason(
        real_execution_allowed_on_this_host,
        auth_configured,
        codex_compatible || json_actions_supported,
    );
    let codex_real_execution_allowed = real_execution_block_reason == "none";
    json!({
        "lane": package
            .model_gateway
            .get("lane")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "selected_model": selected_model,
        "profile_available": package.model_gateway.get("profile").is_some(),
        "profile_source": package
            .model_gateway
            .get("profile_source")
            .cloned()
            .unwrap_or(Value::Null),
        "profile_status": package
            .model_gateway
            .get("profile_status")
            .cloned()
            .unwrap_or(Value::Null),
        "profile_id": profile_id,
        "provider_id": profile.get("provider_id").cloned().unwrap_or(Value::Null),
        "model_id": profile.get("model_id").cloned().unwrap_or(Value::Null),
        "wire_api": wire_api,
        "auth_configured": auth_configured,
        "capabilities": capabilities,
        "capability_manifest": capability_manifest,
        "codex_surface": {
            "wire_api": wire_api,
            "codex_compatible": codex_compatible,
            "json_actions_supported": json_actions_supported,
            "tool_calls_supported": tool_calls_supported,
            "real_execution_allowed_on_this_host": real_execution_allowed_on_this_host,
            "real_execution_block_reason": real_execution_block_reason,
        },
        "codex_real_execution_allowed": codex_real_execution_allowed,
    })
}

fn codex_executor_profile_capabilities(profile: &Value) -> Vec<String> {
    profile
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn codex_executor_empty_capability_manifest() -> Value {
    json!({
        "chat": false,
        "reasoning": false,
        "vision": false,
        "audio": false,
        "video": false,
        "json_mode": false,
        "tool_calling": false,
        "image_prompt": false,
        "static_page": false,
        "codex_compatible": false,
        "extra": [],
    })
}

fn codex_executor_capability_manifest_from_profile(
    profile: &Value,
    capabilities: &[String],
) -> Value {
    json!({
        "chat": codex_executor_capability_present(profile, capabilities, "chat", &["chat", "conversation"]),
        "reasoning": codex_executor_capability_present(profile, capabilities, "reasoning", &["reasoning", "think", "thinking"]),
        "vision": codex_executor_capability_present(profile, capabilities, "vision", &["vision", "document", "vlm"]),
        "audio": codex_executor_capability_present(profile, capabilities, "audio", &["audio", "transcript"]),
        "video": codex_executor_capability_present(profile, capabilities, "video", &["video", "scene"]),
        "json_mode": codex_executor_capability_present(profile, capabilities, "json_mode", &["json", "json_mode", "structured_output"]),
        "tool_calling": codex_executor_capability_present(profile, capabilities, "tool_calling", &["tool", "tools", "tool_calling", "tool_control"]),
        "image_prompt": codex_executor_capability_present(profile, capabilities, "image_prompt", &["image_prompt", "visual_prompt"]),
        "static_page": codex_executor_capability_present(profile, capabilities, "static_page", &["static_page", "static_page_plan", "static_page_edit"]),
        "codex_compatible": codex_executor_capability_present(profile, capabilities, "codex_compatible", &["codex", "codex_compatible", "codex_executor"]),
        "extra": codex_executor_extra_capabilities(profile, capabilities),
    })
}

fn codex_executor_capability_present(
    profile: &Value,
    capabilities: &[String],
    flag_key: &str,
    aliases: &[&str],
) -> bool {
    if profile
        .pointer(&format!("/capability_flags/{flag_key}"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }
    capabilities.iter().any(|capability| {
        aliases
            .iter()
            .any(|alias| capability.eq_ignore_ascii_case(alias))
    })
}

fn codex_executor_extra_capabilities(profile: &Value, capabilities: &[String]) -> Vec<String> {
    if let Some(extra) = profile
        .pointer("/capability_flags/extra")
        .and_then(Value::as_array)
    {
        return extra
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToString::to_string)
            .collect();
    }

    capabilities
        .iter()
        .filter(|capability| {
            !matches!(
                capability.to_ascii_lowercase().as_str(),
                "chat"
                    | "conversation"
                    | "reasoning"
                    | "think"
                    | "thinking"
                    | "vision"
                    | "document"
                    | "vlm"
                    | "audio"
                    | "transcript"
                    | "video"
                    | "scene"
                    | "json"
                    | "json_mode"
                    | "structured_output"
                    | "tool"
                    | "tools"
                    | "tool_calling"
                    | "tool_control"
                    | "image_prompt"
                    | "visual_prompt"
                    | "static_page"
                    | "static_page_plan"
                    | "static_page_edit"
                    | "codex"
                    | "codex_compatible"
                    | "codex_executor"
            )
        })
        .cloned()
        .collect()
}

fn codex_executor_capability_enabled(manifest: &Value, key: &str) -> bool {
    manifest.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn codex_executor_real_execution_block_reason(
    real_execution_allowed_on_this_host: bool,
    auth_configured: bool,
    codex_surface_supported: bool,
) -> &'static str {
    if !real_execution_allowed_on_this_host {
        return "disabled_on_this_host";
    }
    if !auth_configured {
        return "auth_not_configured";
    }
    if !codex_surface_supported {
        return "unsupported_codex_surface";
    }
    "none"
}

fn codex_executor_host_invocation_blueprint(
    package: &AssistantRunCodexContextPackageView,
    output_schema: &Value,
) -> Value {
    let kind = match package.executor_transport {
        AssistantRunExecutorTransportView::CodexExecSchema => "codex_exec_output_schema",
        AssistantRunExecutorTransportView::CodexSdkThread => "codex_sdk_thread",
        AssistantRunExecutorTransportView::CodexAppServer => "codex_app_server",
        AssistantRunExecutorTransportView::CodexMcpServer => "codex_mcp_server",
        _ => "not_applicable",
    };
    json!({
        "kind": kind,
        "transport": package.executor_transport.as_str(),
        "host_required": true,
        "local_execution_allowed": false,
        "mutation_allowed": false,
        "queue_allowed": false,
        "input_contract": "AssistantRunCodexContextPackageView",
        "output_schema_title": output_schema
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("V3CodexPlanOnlyActionSuggestion"),
        "output_schema_required": output_schema
            .get("required")
            .cloned()
            .unwrap_or_else(|| json!(["suggested_action"])),
        "command_blueprint": {
            "program": "codex",
            "args": ["exec", "--output-schema", "<v3-managed-action-schema.json>"],
            "stdin": "serialized AssistantRunCodexContextPackageView",
            "workspace": "host-task-scoped-workspace",
        },
        "model_gateway": codex_executor_model_gateway_summary(package),
        "safety": {
            "v3_validates_all_actions": package.safety.v3_validates_all_actions,
            "direct_database_access_allowed": package.safety.direct_database_access_allowed,
            "direct_queue_access_allowed": package.safety.direct_queue_access_allowed,
            "real_host_validation_required": true,
        }
    })
}

fn codex_executor_action_output_schema(package: &AssistantRunCodexContextPackageView) -> Value {
    let mut action_types = package.action_types();
    action_types.sort();
    action_types.dedup();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "V3CodexPlanOnlyActionSuggestion",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "assistant_message": {
                "type": "string",
                "description": "User-facing answer text when no V3 action is needed."
            },
            "suggested_action": {
                "type": ["object", "null"],
                "additionalProperties": false,
                "properties": {
                    "action_type": {
                        "type": "string",
                        "enum": action_types
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments must match the selected V3 action contract and remain subject to V3 validation."
                    },
                    "reason": {"type": "string"},
                    "confidence": {"type": "string", "enum": ["low", "medium", "high"]},
                    "requires_confirmation": {"type": "boolean"},
                    "external_action_policy": {
                        "type": ["object", "null"],
                        "description": "Risk and confirmation policy for external artifact or business actions."
                    }
                },
                "required": ["action_type", "reason"]
            },
            "progress": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "label": {"type": "string"},
                        "status": {"type": "string"}
                    },
                    "required": ["label", "status"]
                }
            }
        },
        "required": ["suggested_action"]
    })
}

fn codex_executor_suggest_action(package: &AssistantRunCodexContextPackageView) -> Option<Value> {
    let action_type = codex_executor_suggest_action_type(package)?;
    let contract = package
        .available_actions
        .iter()
        .find(|action| action.action_type == action_type)?;
    Some(codex_executor_action_suggestion(
        package,
        contract,
        codex_executor_suggestion_reason(&action_type),
    ))
}

fn codex_executor_suggest_action_type(
    package: &AssistantRunCodexContextPackageView,
) -> Option<String> {
    let available = package
        .available_actions
        .iter()
        .map(|action| action.action_type.as_str())
        .collect::<HashSet<_>>();
    let supply_status = codex_executor_supply_status(package);
    let static_page_context = codex_executor_scope_intent(package) == "static_page"
        || package
            .current_artifact
            .as_ref()
            .is_some_and(codex_executor_is_static_page_artifact);

    if matches!(supply_status.as_str(), "missing" | "partial")
        && available.contains("read_document_detail")
        && codex_executor_has_detail_targets(package)
    {
        return Some("read_document_detail".to_string());
    }
    if matches!(supply_status.as_str(), "missing")
        && available.contains("retrieve_evidence")
        && package.context_budget.selected_dataset_count > 0
    {
        return Some("retrieve_evidence".to_string());
    }
    if package
        .current_artifact
        .as_ref()
        .is_some_and(codex_executor_is_html_artifact)
        && codex_executor_prompt_requests_artifact_edit(&package.user_prompt)
        && available.contains("submit_html_artifact_event")
    {
        return Some("submit_html_artifact_event".to_string());
    }
    if static_page_context
        && package.current_artifact.is_none()
        && available.contains("create_static_page_draft")
    {
        return Some("create_static_page_draft".to_string());
    }
    if static_page_context
        && codex_executor_prompt_requests_preview(&package.user_prompt)
        && available.contains("submit_static_page_image_preview")
    {
        if codex_executor_static_page_needs_data_quality_attention(package) {
            if available.contains("update_static_page_module") && package.current_artifact.is_some()
            {
                return Some("update_static_page_module".to_string());
            }
            if available.contains("retrieve_evidence")
                && package.context_budget.selected_dataset_count > 0
            {
                return Some("retrieve_evidence".to_string());
            }
        }
        return Some("submit_static_page_image_preview".to_string());
    }
    if static_page_context
        && package.current_artifact.is_some()
        && available.contains("update_static_page_module")
    {
        return Some("update_static_page_module".to_string());
    }
    if codex_executor_scope_intent(package) == "report" && available.contains("create_report_draft")
    {
        return Some("create_report_draft".to_string());
    }
    let recommended_actions = codex_executor_scope_recommended_actions(package);
    if recommended_actions.contains("media.resolve_video_url")
        && available.contains("resolve_video_url")
    {
        return Some("resolve_video_url".to_string());
    }
    if recommended_actions.contains("media.extract_ppt_transcript")
        && available.contains("extract_video_ppt_transcript")
    {
        return Some("extract_video_ppt_transcript".to_string());
    }
    if let Some(action_type) = codex_executor_external_action_type(package, &available) {
        return Some(action_type);
    }
    if codex_executor_prompt_requests_web_search(&package.user_prompt)
        && available.contains("web_search")
    {
        return Some("web_search".to_string());
    }
    if available.contains("final_answer") {
        return Some("final_answer".to_string());
    }
    None
}

fn codex_executor_scope_recommended_actions(
    package: &AssistantRunCodexContextPackageView,
) -> HashSet<&str> {
    package
        .selected_scope
        .get("supply_policy")
        .or_else(|| package.selected_scope.get("supplyPolicy"))
        .and_then(|policy| {
            policy
                .get("recommendedActions")
                .or_else(|| policy.get("recommended_actions"))
        })
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default()
}

fn codex_executor_external_action_type(
    package: &AssistantRunCodexContextPackageView,
    available: &HashSet<&str>,
) -> Option<String> {
    if !codex_executor_is_external_channel_scope(package) {
        return None;
    }
    let recommended_actions = codex_executor_scope_recommended_actions(package);
    for action_type in [
        "external_artifact.status",
        "external_artifact.publish",
        "external_artifact.revoke",
        "external_business_action.invoke",
    ] {
        if recommended_actions.contains(action_type) && available.contains(action_type) {
            return Some(action_type.to_string());
        }
    }

    let prompt = package.user_prompt.as_str();
    if codex_executor_prompt_requests_external_status(prompt)
        && available.contains("external_artifact.status")
    {
        return Some("external_artifact.status".to_string());
    }
    if codex_executor_prompt_requests_external_revoke(prompt)
        && package.current_artifact.is_some()
        && available.contains("external_artifact.revoke")
    {
        return Some("external_artifact.revoke".to_string());
    }
    if codex_executor_prompt_requests_external_publish(prompt)
        && package.current_artifact.is_some()
        && available.contains("external_artifact.publish")
    {
        return Some("external_artifact.publish".to_string());
    }
    if codex_executor_prompt_requests_external_business_action(prompt)
        && available.contains("external_business_action.invoke")
    {
        return Some("external_business_action.invoke".to_string());
    }
    None
}

fn codex_executor_action_suggestion(
    package: &AssistantRunCodexContextPackageView,
    contract: &AssistantRunCodexActionContractView,
    reason: &'static str,
) -> Value {
    let external_action_policy =
        codex_executor_external_action_policy(package, &contract.action_type);
    let requires_confirmation = external_action_policy
        .get("requires_confirmation")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    json!({
        "action_type": contract.action_type.clone(),
        "title": contract.title.clone(),
        "reason": reason,
        "source": "codex_plan_only_shadow",
        "confidence": codex_executor_suggestion_confidence(package, &contract.action_type),
        "requires_confirmation": requires_confirmation,
        "external_action_policy": external_action_policy,
        "requires_v3_validation": contract.requires_v3_validation,
        "mutates_state": contract.mutates_state,
        "mutation_allowed": false,
        "queue_allowed": false,
        "arguments": codex_executor_suggestion_arguments(package, &contract.action_type),
        "input_schema": contract.input_schema.clone(),
    })
}

fn codex_executor_suggestion_arguments(
    package: &AssistantRunCodexContextPackageView,
    action_type: &str,
) -> Value {
    match action_type {
        "read_document_detail" => json!({
            "detail_targets": package
                .evidence_state
                .get("detail_targets")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "reason": "detail-first supply requested by V3 context",
        }),
        "retrieve_evidence" => json!({
            "query": package.user_prompt,
            "selected_scope": package.selected_scope,
            "reason": "selected scope has missing supply",
        }),
        "web_search" => json!({
            "query": package.user_prompt,
            "reason": "user requested current, live, or web-sourced information outside supplied V3 evidence",
            "freshness": codex_executor_web_search_freshness(&package.user_prompt),
            "language": "auto",
            "evidence_contract": {
                "requires_source_url": true,
                "requires_source_title": true,
                "requires_retrieved_at": true,
                "requires_query_metadata": true,
                "must_enter_v3_search_evidence_before_citation": true
            },
            "source": "codex_plan_only_shadow",
        }),
        "create_static_page_draft" => json!({
            "prompt": package.user_prompt,
            "selected_scope": package.selected_scope,
            "source": "codex_plan_only_shadow",
        }),
        "submit_static_page_image_preview" => json!({
            "draft_id": codex_executor_artifact_id(package.current_artifact.as_ref()),
            "source": "codex_plan_only_shadow",
            "requires_customer_confirmation": true,
        }),
        "submit_html_artifact_event" => json!({
            "artifact_id": codex_executor_artifact_id(package.current_artifact.as_ref()),
            "event_type": "html_artifact.action_intent",
            "payload": {
                "prompt": package.user_prompt,
                "source": "codex_plan_only_shadow"
            },
        }),
        "update_static_page_module" => json!({
            "draft_id": codex_executor_artifact_id(package.current_artifact.as_ref()),
            "prompt": package.user_prompt,
            "data_quality_gate": codex_executor_static_page_quality_gate(package),
            "source": "codex_plan_only_shadow",
        }),
        "render_static_page" => json!({
            "draft_id": codex_executor_artifact_id(package.current_artifact.as_ref()),
            "source": "codex_plan_only_shadow",
        }),
        "create_report_draft" => json!({
            "prompt": package.user_prompt,
            "selected_scope": package.selected_scope,
            "source": "codex_plan_only_shadow",
        }),
        "recall_conversation_memory" => json!({
            "query": package.user_prompt,
            "local_thread_id": package.local_thread_id,
            "source": "codex_plan_only_shadow",
        }),
        "resolve_video_url" => json!({
            "prompt": package.user_prompt,
            "selected_scope": package.selected_scope,
            "allowed_source_types": ["direct_video_url", "public_page_resolvable_video"],
            "disallowed_source_types": ["login_gated_page", "qr_login", "cookies", "screen_recording_bypass"],
            "source": "codex_plan_only_shadow",
        }),
        "extract_video_ppt_transcript" => json!({
            "prompt": package.user_prompt,
            "selected_scope": package.selected_scope,
            "required_asset_state": "uploaded_or_resolved_video",
            "deliverables": ["transcript_text", "slide_image_candidates", "ppt_outline_or_pptx", "timestamp_map"],
            "source": "codex_plan_only_shadow",
        }),
        "external_artifact.status" => codex_executor_external_action_arguments(
            package,
            "external_artifact.status",
            "artifact_status",
        ),
        "external_artifact.publish" => codex_executor_external_action_arguments(
            package,
            "external_artifact.publish",
            "artifact_publish",
        ),
        "external_artifact.revoke" => codex_executor_external_action_arguments(
            package,
            "external_artifact.revoke",
            "artifact_revoke",
        ),
        "external_business_action.invoke" => codex_executor_external_action_arguments(
            package,
            "external_business_action.invoke",
            "business_action",
        ),
        "final_answer" => codex_executor_final_answer_arguments(package),
        _ => json!({
            "source": "codex_plan_only_shadow",
        }),
    }
}

fn codex_executor_final_answer_arguments(package: &AssistantRunCodexContextPackageView) -> Value {
    json!({
        "mode": "model_authored_answer",
        "source": "codex_plan_only_shadow",
        "v3_context_contract": {
            "identity": "AI Data Platform V3 supplies product, permission, dataset, evidence, tool, report, and static-page context.",
            "additive_context_not_capability_limit": true,
            "ordinary_chat_allowed_without_v3_evidence": true,
            "unavailable_evidence_phrase": "当前不可见/未供料",
            "unavailable_evidence_rule": "Use the phrase only when the answer depends on V3 data, documents, permissions, tool results, artifact state, or live/search evidence that V3 has not supplied; after that, the model may continue with clearly labeled general knowledge or assumptions.",
            "external_search_rule": "Do not claim web search, current news, or cite live web results unless V3 supplied audited search evidence with source and retrieved_at metadata.",
            "no_host_composed_answer": true,
        },
        "v3_context_state": {
            "intent": codex_executor_scope_intent(package),
            "selected_dataset_count": package.context_budget.selected_dataset_count,
            "evidence_status": codex_executor_evidence_status(package),
            "supply_quality_status": codex_executor_supply_status(package),
            "search_evidence_supplied": codex_executor_search_evidence_supplied(package),
        },
    })
}

fn codex_executor_external_action_arguments(
    package: &AssistantRunCodexContextPackageView,
    action_type: &str,
    business_action_type: &str,
) -> Value {
    let policy = codex_executor_external_action_policy(package, action_type);
    let risk_level = policy
        .get("risk_level")
        .and_then(Value::as_str)
        .unwrap_or("read_only");
    let requires_confirmation = policy
        .get("requires_confirmation")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let artifact_ref = codex_executor_artifact_id(package.current_artifact.as_ref());
    json!({
        "connection_id": codex_executor_external_connection_id(package),
        "target_system": codex_executor_external_target_system(package),
        "artifact_ref": artifact_ref,
        "business_action_type": business_action_type,
        "risk_level": risk_level,
        "requires_confirmation": requires_confirmation,
        "confirmation_mode": policy
            .get("confirmation_mode")
            .cloned()
            .unwrap_or_else(|| json!("not_required")),
        "arguments_redacted": codex_executor_external_arguments_redacted(
            package,
            action_type,
            business_action_type
        ),
        "source_evidence_refs": codex_executor_external_source_evidence_refs(package),
        "idempotency_key": format!(
            "{}:{}:{}",
            codex_executor_external_connection_id(package),
            action_type,
            codex_executor_external_message_id(package)
        ),
        "source": "codex_plan_only_shadow",
    })
}

fn codex_executor_external_action_policy(
    package: &AssistantRunCodexContextPackageView,
    action_type: &str,
) -> Value {
    if !codex_executor_is_external_channel_scope(package) {
        return Value::Null;
    }
    let (capability, risk_level, requires_confirmation, confirmation_mode) = match action_type {
        "external_artifact.status" => ("artifact_status", "read_only", false, "not_required"),
        "external_artifact.publish" => {
            ("artifact_publish", "low_risk_write", false, "not_required")
        }
        "external_artifact.revoke" => (
            "artifact_revoke",
            "high_risk_write",
            true,
            "original_channel",
        ),
        "external_business_action.invoke" => (
            "business_action",
            "cross_system",
            true,
            "trusted_customer_page",
        ),
        _ => return Value::Null,
    };
    json!({
        "capability": capability,
        "action_type": action_type,
        "target_system": codex_executor_external_target_system(package),
        "risk_level": risk_level,
        "requires_confirmation": requires_confirmation,
        "confirmation_mode": confirmation_mode,
        "confirmation_boundary": if requires_confirmation {
            "original_channel_or_trusted_customer_page"
        } else {
            "not_required"
        },
        "raw_arguments_allowed": false,
        "v3_persists_external_action_run": true,
    })
}

fn codex_executor_is_external_channel_scope(package: &AssistantRunCodexContextPackageView) -> bool {
    package.selected_scope.get("type").and_then(Value::as_str) == Some("external_channel")
        || package
            .startup_briefing
            .get("surface")
            .and_then(Value::as_str)
            == Some("external_channel")
}

fn codex_executor_external_connection_id(package: &AssistantRunCodexContextPackageView) -> String {
    package
        .selected_scope
        .get("channel_connection_id")
        .or_else(|| package.selected_scope.get("channelConnectionId"))
        .and_then(Value::as_str)
        .unwrap_or("external-channel")
        .to_string()
}

fn codex_executor_external_target_system(package: &AssistantRunCodexContextPackageView) -> String {
    package
        .selected_scope
        .get("target_system")
        .or_else(|| package.selected_scope.get("targetSystem"))
        .and_then(Value::as_str)
        .unwrap_or("external_channel")
        .to_string()
}

fn codex_executor_external_message_id(package: &AssistantRunCodexContextPackageView) -> String {
    package
        .selected_scope
        .get("message_external_id")
        .or_else(|| package.selected_scope.get("messageExternalId"))
        .and_then(Value::as_str)
        .unwrap_or("message")
        .to_string()
}

fn codex_executor_external_arguments_redacted(
    package: &AssistantRunCodexContextPackageView,
    action_type: &str,
    business_action_type: &str,
) -> Value {
    json!({
        "action_type": action_type,
        "business_action_type": business_action_type,
        "prompt_chars": package.user_prompt.chars().count(),
        "artifact_ref_present": package.current_artifact.is_some(),
        "raw_prompt_redacted": true,
    })
}

fn codex_executor_external_source_evidence_refs(
    package: &AssistantRunCodexContextPackageView,
) -> Vec<String> {
    package
        .evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("retrieval_evidence_id")
                        .or_else(|| item.get("retrievalEvidenceId"))
                        .or_else(|| item.get("evidence_ref"))
                        .or_else(|| item.get("evidenceRef"))
                        .and_then(Value::as_str)
                })
                .take(8)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn codex_executor_evidence_status(package: &AssistantRunCodexContextPackageView) -> String {
    if let Some(status) = package.evidence_state.get("status").and_then(Value::as_str) {
        return status.to_string();
    }
    let supply_status = codex_executor_supply_status(package);
    match supply_status.as_str() {
        "not_requested" => "not_requested".to_string(),
        "grounded" | "supplied" | "partial" => "supplied".to_string(),
        "missing" => "missing".to_string(),
        _ => "unknown".to_string(),
    }
}

fn codex_executor_search_evidence_supplied(package: &AssistantRunCodexContextPackageView) -> bool {
    package
        .evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                let item_type = item
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let source = item
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                item_type.contains("search")
                    || source.contains("web_search")
                    || source.contains("search")
                    || item.get("search_evidence_id").is_some()
                    || item.get("searchEvidenceId").is_some()
            })
        })
}

fn codex_executor_suggestion_reason(action_type: &str) -> &'static str {
    match action_type {
        "read_document_detail" => {
            "partial supply prefers detail read before high-confidence claims"
        }
        "retrieve_evidence" => "selected scope needs V3 retrieval before grounded answer",
        "web_search" => {
            "user requested live or web-sourced information; V3 search evidence is required before citation"
        }
        "create_static_page_draft" => "static-page intent has no current draft in context",
        "submit_static_page_image_preview" => {
            "user prompt asks for effect preview from current draft"
        }
        "submit_html_artifact_event" => {
            "current HTML artifact change must go through V3 validated artifact event"
        }
        "update_static_page_module" => {
            "current static-page draft can be revised through V3 operations"
        }
        "create_report_draft" => "report intent can be handled by V3 report draft flow",
        "resolve_video_url" => {
            "video PPT extraction starts with V3-controlled public/direct video resolution"
        }
        "extract_video_ppt_transcript" => {
            "video asset is the required input for transcript and PPT extraction"
        }
        "external_artifact.status" => {
            "external artifact status is a read-only V3-controlled action"
        }
        "external_artifact.publish" => {
            "external artifact publication must be issued through the V3 action boundary"
        }
        "external_artifact.revoke" => {
            "external artifact revocation is high-risk and requires channel confirmation"
        }
        "external_business_action.invoke" => {
            "cross-system external business actions require V3 validation and confirmation"
        }
        "final_answer" => "no platform action is required for this turn",
        _ => "available V3 action contract selected by plan-only executor",
    }
}

fn codex_executor_suggestion_confidence(
    package: &AssistantRunCodexContextPackageView,
    action_type: &str,
) -> &'static str {
    if action_type == "final_answer" {
        return "medium";
    }
    if codex_executor_scope_intent(package) == "static_page"
        || package
            .current_artifact
            .as_ref()
            .is_some_and(codex_executor_is_static_page_artifact)
    {
        return "high";
    }
    "medium"
}

fn codex_executor_supply_status(package: &AssistantRunCodexContextPackageView) -> String {
    let source = if package.supply_quality.is_null() {
        package
            .evidence_state
            .get("supply_quality")
            .or_else(|| package.evidence_state.get("supplyQuality"))
            .unwrap_or(&Value::Null)
    } else {
        &package.supply_quality
    };
    source
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn codex_executor_scope_intent(package: &AssistantRunCodexContextPackageView) -> &str {
    package
        .selected_scope
        .get("intent")
        .and_then(Value::as_str)
        .unwrap_or("ordinary_chat")
}

fn codex_executor_has_detail_targets(package: &AssistantRunCodexContextPackageView) -> bool {
    package
        .evidence_state
        .get("detail_targets")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn codex_executor_static_page_needs_data_quality_attention(
    package: &AssistantRunCodexContextPackageView,
) -> bool {
    package
        .current_artifact
        .as_ref()
        .and_then(codex_executor_static_page_binding_quality)
        .is_some_and(codex_executor_binding_quality_needs_attention)
}

fn codex_executor_static_page_quality_gate(package: &AssistantRunCodexContextPackageView) -> Value {
    let Some(binding_quality) = package
        .current_artifact
        .as_ref()
        .and_then(codex_executor_static_page_binding_quality)
    else {
        return Value::Null;
    };

    let modules = binding_quality
        .get("modules")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|module| codex_executor_binding_quality_module_needs_attention(module))
                .take(8)
                .map(|module| {
                    json!({
                        "module_id": codex_executor_quality_string(module, &["module_id", "moduleId", "id"]),
                        "title": codex_executor_quality_string(module, &["title", "module_title", "moduleTitle"]),
                        "binding_quality_status": codex_executor_quality_string(module, &["bindingQualityStatus", "binding_quality_status", "status"]),
                        "chart_data_fit": codex_executor_quality_string(module, &["chartDataFit", "chart_data_fit"]),
                        "sample_rows": codex_executor_quality_u64(module, &["sampleRows", "sample_rows"]),
                        "recommended_action": codex_executor_quality_string(module, &["recommendedAction", "recommended_action"]),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "has_attention": codex_executor_binding_quality_needs_attention(binding_quality),
        "attention_modules": codex_executor_quality_u64(binding_quality, &["attentionModules", "attention_modules"]),
        "modules": modules,
    })
}

fn codex_executor_static_page_binding_quality(artifact: &Value) -> Option<&Value> {
    artifact
        .get("bindingQuality")
        .or_else(|| artifact.get("binding_quality"))
}

fn codex_executor_binding_quality_needs_attention(binding_quality: &Value) -> bool {
    if codex_executor_quality_u64(binding_quality, &["attentionModules", "attention_modules"]) > 0 {
        return true;
    }
    binding_quality
        .get("modules")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(codex_executor_binding_quality_module_needs_attention)
        })
}

fn codex_executor_binding_quality_module_needs_attention(module: &Value) -> bool {
    let status = codex_executor_quality_string(
        module,
        &["bindingQualityStatus", "binding_quality_status", "status"],
    );
    if !status.is_empty() && !matches!(status.as_str(), "confirmed" | "ready" | "non_chart") {
        return true;
    }

    let chart_data_fit = codex_executor_quality_string(module, &["chartDataFit", "chart_data_fit"]);
    if !chart_data_fit.is_empty()
        && !matches!(
            chart_data_fit.as_str(),
            "ready" | "not_required" | "non_chart_ready"
        )
    {
        return true;
    }

    false
}

fn codex_executor_quality_string(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

fn codex_executor_quality_u64(value: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(|item| item.as_u64().or_else(|| item.as_str()?.parse::<u64>().ok()))
        })
        .unwrap_or(0)
}

fn codex_executor_is_static_page_artifact(artifact: &Value) -> bool {
    let kind = artifact
        .get("kind")
        .or_else(|| artifact.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    kind.contains("static_page")
        || artifact.get("backendDraftId").is_some()
        || artifact.get("draft_id").is_some()
        || artifact.get("draftId").is_some()
}

fn codex_executor_is_html_artifact(artifact: &Value) -> bool {
    let kind = artifact
        .get("kind")
        .or_else(|| artifact.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    kind == "html_artifact"
        || artifact.get("template_id").is_some()
        || artifact.get("templateId").is_some()
        || artifact.get("interaction_mode").is_some()
        || artifact.get("interactionMode").is_some()
}

fn codex_executor_artifact_id(artifact: Option<&Value>) -> Value {
    artifact
        .and_then(|artifact| {
            artifact
                .get("id")
                .or_else(|| artifact.get("artifact_id"))
                .or_else(|| artifact.get("artifactId"))
                .or_else(|| artifact.get("backendDraftId"))
                .or_else(|| artifact.get("draft_id"))
                .or_else(|| artifact.get("draftId"))
                .and_then(Value::as_str)
        })
        .map(|value| json!(value))
        .unwrap_or(Value::Null)
}

fn codex_executor_prompt_requests_preview(prompt: &str) -> bool {
    ["效果图", "预览图", "出图", "生成图", "image preview"]
        .iter()
        .any(|hint| prompt.contains(hint))
}

fn codex_executor_prompt_requests_artifact_edit(prompt: &str) -> bool {
    [
        "修改", "调整", "应用", "提交", "更新", "改成", "换成", "patch", "apply", "submit",
        "update",
    ]
    .iter()
    .any(|hint| prompt.contains(hint))
}

fn codex_executor_prompt_requests_web_search(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    [
        "联网",
        "网页搜索",
        "搜索一下",
        "上网查",
        "查最新",
        "最新消息",
        "实时",
        "今天",
        "新闻",
        "web search",
        "search the web",
        "browse",
        "latest",
        "current",
        "today",
        "news",
    ]
    .iter()
    .any(|hint| lower.contains(hint))
}

fn codex_executor_web_search_freshness(prompt: &str) -> &'static str {
    let lower = prompt.to_ascii_lowercase();
    if ["最新", "实时", "今天", "latest", "current", "today", "news"]
        .iter()
        .any(|hint| lower.contains(hint))
    {
        "latest"
    } else {
        "unspecified"
    }
}

fn codex_executor_prompt_requests_external_status(prompt: &str) -> bool {
    [
        "状态",
        "进度",
        "查一下",
        "查询",
        "是否发布",
        "status",
        "check",
    ]
    .iter()
    .any(|hint| prompt.contains(hint))
}

fn codex_executor_prompt_requests_external_publish(prompt: &str) -> bool {
    ["发布", "发送", "同步", "上线", "公开", "publish", "release"]
        .iter()
        .any(|hint| prompt.contains(hint))
}

fn codex_executor_prompt_requests_external_revoke(prompt: &str) -> bool {
    [
        "撤回",
        "撤销",
        "取消发布",
        "下线",
        "停用",
        "revoke",
        "unpublish",
    ]
    .iter()
    .any(|hint| prompt.contains(hint))
}

fn codex_executor_prompt_requests_external_business_action(prompt: &str) -> bool {
    [
        "创建",
        "更新",
        "处理",
        "办理",
        "发起",
        "提交",
        "开通",
        "关闭",
        "审批",
        "退款",
        "工单",
        "business action",
        "invoke",
    ]
    .iter()
    .any(|hint| prompt.contains(hint))
}

pub fn plan_scope(input: ScopePlannerInput<'_>) -> ScopePlan {
    let prompt = input.prompt.trim();
    let mut candidates = Vec::new();
    let intent = infer_assistant_intent(prompt);
    let should_match_datasets = intent != "ordinary_chat";

    if let Some(selected_dataset_id) = input.selected_dataset_id {
        if let Some(dataset) = input
            .visible_datasets
            .iter()
            .find(|dataset| dataset.id == selected_dataset_id)
        {
            candidates.push(dataset_scope_candidate(
                dataset,
                ScopeConfidence::High,
                "用户当前已选中该供料范围",
                "user_selected",
            ));
        }
    }

    if should_match_datasets {
        for dataset in input.visible_datasets {
            if input.selected_dataset_id == Some(dataset.id) {
                continue;
            }
            let haystack = dataset_haystack(dataset);
            let matched_by_name = text_matches(prompt, &haystack);
            let matched_by_hint = BUSINESS_HINTS.iter().any(|(label, hints)| {
                haystack.contains(label) && hints.iter().any(|hint| prompt.contains(hint))
            });
            if matched_by_name || matched_by_hint {
                candidates.push(dataset_scope_candidate(
                    dataset,
                    if matched_by_name {
                        ScopeConfidence::High
                    } else {
                        ScopeConfidence::Medium
                    },
                    if matched_by_name {
                        "用户提到数据集名称或关键字"
                    } else {
                        "用户问题命中常用业务主题"
                    },
                    "scope_planner",
                ));
            }
        }
    }

    if input.conversation_memory_available
        && CONVERSATION_HINTS.iter().any(|hint| prompt.contains(hint))
    {
        candidates.push(ScopeCandidate {
            candidate_type: ScopeCandidateType::ConversationMemory,
            id: "local-thread".to_string(),
            label: "本轮对话历史".to_string(),
            key: String::new(),
            visibility: String::new(),
            category: String::new(),
            lifecycle: String::new(),
            document_count: 0,
            estimated_word_count: 0,
            parse_status_summary: String::new(),
            latest_activity: String::new(),
            confidence: ScopeConfidence::Medium,
            reason: "用户引用了刚才或已有草稿内容".to_string(),
            source: "scope_planner".to_string(),
            material_hints: Vec::new(),
            noun_term_hints: Vec::new(),
            section_title_hints: Vec::new(),
        });
    }

    let candidates = dedupe_candidates(candidates)
        .into_iter()
        .take(4)
        .collect::<Vec<_>>();
    let selected_scope = selected_scope_from_candidates(&candidates, intent, prompt);
    let hint = build_scope_hint(&candidates, intent);

    ScopePlan {
        candidates,
        selected_scope,
        hint,
        intent: intent.to_string(),
    }
}

fn unsafe_codex_context_reason(
    package: &AssistantRunCodexContextPackageView,
) -> Option<&'static str> {
    if !package.safety.v3_validates_all_actions {
        return Some("v3_action_validation_required");
    }
    if package.safety.direct_database_access_allowed {
        return Some("direct_database_access_forbidden");
    }
    if package.safety.direct_queue_access_allowed {
        return Some("direct_queue_access_forbidden");
    }
    if package.safety.direct_filesystem_access_allowed {
        return Some("direct_filesystem_access_forbidden");
    }
    if package.safety.provider_keys_in_prompt_allowed {
        return Some("provider_keys_in_prompt_forbidden");
    }
    if !package.safety.raw_logs_require_redaction {
        return Some("raw_log_redaction_required");
    }
    if !package.safety.static_page_state_machine_owned_by_v3 {
        return Some("static_page_state_machine_must_remain_v3_owned");
    }
    None
}

pub fn candidates_to_values(candidates: &[ScopeCandidate]) -> Vec<Value> {
    candidates
        .iter()
        .filter_map(|candidate| serde_json::to_value(candidate).ok())
        .collect()
}

fn selected_scope_from_candidates(
    candidates: &[ScopeCandidate],
    intent: &str,
    prompt: &str,
) -> Value {
    let conversation_memory = conversation_memory_scope_from_candidates(candidates);
    let has_memory = conversation_memory
        .as_array()
        .map(|items| !items.is_empty())
        .unwrap_or(false);
    let detail_prompt = prompt_has_media_detail(prompt);
    let video_ppt_extraction = prompt_wants_video_ppt_extraction(prompt);
    let direct_video_source = prompt_has_direct_video_source(prompt);

    if let Some(dataset) = candidates.iter().find(|candidate| {
        candidate.candidate_type == ScopeCandidateType::Dataset
            && candidate.source == "user_selected"
    }) {
        return json!({
            "mode": "user_selected",
            "datasets": [dataset.id],
            "conversation_memory": conversation_memory,
            "intent": intent,
            "supply_policy": supply_policy_for_scope(intent, true, has_memory, detail_prompt, video_ppt_extraction, direct_video_source),
        });
    }

    if let Some(dataset) = candidates.iter().find(|candidate| {
        candidate.candidate_type == ScopeCandidateType::Dataset
            && matches!(
                candidate.confidence,
                ScopeConfidence::High | ScopeConfidence::Medium
            )
    }) {
        return json!({
            "mode": "preselected",
            "datasets": [dataset.id],
            "conversation_memory": conversation_memory,
            "reason": dataset.reason,
            "intent": intent,
            "supply_policy": supply_policy_for_scope(intent, true, has_memory, detail_prompt, video_ppt_extraction, direct_video_source),
        });
    }

    json!({
        "mode": "ordinary_chat",
        "datasets": [],
        "conversation_memory": conversation_memory,
        "intent": intent,
        "supply_policy": supply_policy_for_scope(intent, false, has_memory, detail_prompt, video_ppt_extraction, direct_video_source),
    })
}

fn conversation_memory_scope_from_candidates(candidates: &[ScopeCandidate]) -> Value {
    let memory_ids = candidates
        .iter()
        .filter(|candidate| candidate.candidate_type == ScopeCandidateType::ConversationMemory)
        .map(|candidate| candidate.id.as_str())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    json!(memory_ids)
}

fn dataset_label(dataset: &Dataset) -> String {
    if !dataset.title.trim().is_empty() {
        dataset.title.clone()
    } else {
        dataset.key.clone()
    }
}

fn dataset_scope_candidate(
    dataset: &Dataset,
    confidence: ScopeConfidence,
    reason: &str,
    source: &str,
) -> ScopeCandidate {
    let mut latest_activity = dataset_metadata_string(
        dataset,
        &["latestUpload", "latest_upload", "latestActivity"],
    );
    if latest_activity.trim().is_empty() {
        latest_activity = dataset.updated_at.to_rfc3339();
    }

    ScopeCandidate {
        candidate_type: ScopeCandidateType::Dataset,
        id: dataset.id.to_string(),
        label: dataset_label(dataset),
        key: dataset.key.clone(),
        visibility: dataset.visibility.as_str().to_string(),
        category: dataset_metadata_string(dataset, &["category", "default_category"]),
        lifecycle: dataset.lifecycle.as_str().to_string(),
        document_count: dataset_metadata_usize(
            dataset,
            &[
                "document_count",
                "documentCount",
                "documents_count",
                "documentsCount",
            ],
        ),
        estimated_word_count: dataset_metadata_usize(
            dataset,
            &[
                "estimated_word_count",
                "estimatedWordCount",
                "word_count",
                "wordCount",
            ],
        ),
        parse_status_summary: dataset_metadata_string(
            dataset,
            &[
                "parse_status_summary",
                "parseStatusSummary",
                "parse_status",
                "parseStatus",
            ],
        )
        .chars()
        .take(80)
        .collect(),
        latest_activity: latest_activity.chars().take(80).collect(),
        confidence,
        reason: reason.to_string(),
        source: source.to_string(),
        material_hints: dataset_material_hints(dataset),
        noun_term_hints: dataset_metadata_string_list(
            dataset,
            &[
                "noun_term_hints",
                "nounTermHints",
                "noun_terms",
                "nounTerms",
            ],
        )
        .into_iter()
        .take(12)
        .collect(),
        section_title_hints: dataset_metadata_string_list(
            dataset,
            &[
                "section_title_hints",
                "sectionTitleHints",
                "document_section_hints",
                "documentSectionHints",
            ],
        )
        .into_iter()
        .take(12)
        .collect(),
    }
}

fn dataset_metadata_usize(dataset: &Dataset, keys: &[&str]) -> usize {
    keys.iter()
        .find_map(|key| {
            dataset.metadata.get(*key).and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str()?.parse::<u64>().ok())
            })
        })
        .map(|value| value as usize)
        .unwrap_or(0)
}

fn dataset_metadata_string(dataset: &Dataset, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            dataset
                .metadata
                .get(*key)
                .and_then(|value| value.as_str().map(str::trim))
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("")
        .to_string()
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn dataset_haystack(dataset: &Dataset) -> String {
    let document_title_hints =
        dataset_metadata_string_list(dataset, &["document_title_hints", "documentTitleHints"])
            .join(" ");
    let noun_term_hints = dataset_metadata_string_list(
        dataset,
        &[
            "noun_term_hints",
            "nounTermHints",
            "noun_terms",
            "nounTerms",
        ],
    )
    .join(" ");
    let section_title_hints = dataset_metadata_string_list(
        dataset,
        &[
            "section_title_hints",
            "sectionTitleHints",
            "document_section_hints",
            "documentSectionHints",
        ],
    )
    .join(" ");
    let material_hints =
        dataset_metadata_string_list(dataset, &["material_hints", "materialHints"])
            .into_iter()
            .flat_map(|hint| {
                let mut values = vec![hint.clone()];
                values.extend(
                    dataset_material_hint_labels(&hint)
                        .into_iter()
                        .map(str::to_string),
                );
                values
            })
            .collect::<Vec<_>>()
            .join(" ");
    format!(
        "{} {} {} {} {} {} {} {} {} {}",
        dataset.title,
        dataset.key,
        dataset.description.clone().unwrap_or_default(),
        dataset_metadata_string(dataset, &["category", "default_category"]),
        document_title_hints,
        noun_term_hints,
        section_title_hints,
        dataset_metadata_string(dataset, &["content_type_summary", "contentTypeSummary"]),
        dataset_metadata_string(dataset, &["parse_status_summary", "parseStatusSummary"]),
        material_hints
    )
}

fn dataset_material_hints(dataset: &Dataset) -> Vec<String> {
    let haystack = dataset_haystack(dataset);
    let mut hints = dataset_metadata_string_list(dataset, &["material_hints", "materialHints"]);
    if MEDIA_HINTS.iter().any(|hint| haystack.contains(hint)) {
        hints.push("audio_video".to_string());
        hints.push("transcript_possible".to_string());
        hints.push("keyframe_ocr_possible".to_string());
    }
    let mut seen = HashSet::new();
    hints
        .into_iter()
        .filter(|hint| !hint.trim().is_empty())
        .filter(|hint| seen.insert(hint.clone()))
        .take(8)
        .collect()
}

fn dataset_material_hint_labels(hint: &str) -> Vec<&'static str> {
    match hint {
        "audio_video" => vec!["音视频", "音频", "视频", "录音", "会议"],
        "transcript_possible" => vec!["转写", "字幕"],
        "keyframe_ocr_possible" => vec!["关键帧", "OCR"],
        "scene_possible" => vec!["场景"],
        _ => Vec::new(),
    }
}

fn dataset_metadata_string_list(dataset: &Dataset, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| dataset.metadata.get(*key))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn text_matches(prompt: &str, text: &str) -> bool {
    text.split(|ch: char| ch.is_whitespace() || ",，。:：/\\|_-.()（）".contains(ch))
        .map(str::trim)
        .filter(|token| token.chars().count() >= 2)
        .any(|token| prompt.contains(token))
}

fn dedupe_candidates(candidates: Vec<ScopeCandidate>) -> Vec<ScopeCandidate> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(format!("{:?}:{}", candidate.candidate_type, candidate.id)))
        .collect()
}

fn build_scope_hint(candidates: &[ScopeCandidate], intent: &str) -> String {
    let labels = candidates
        .iter()
        .map(format_candidate_hint)
        .filter(|label| !label.is_empty())
        .take(3)
        .collect::<Vec<_>>();
    let mut parts = Vec::new();
    if !labels.is_empty() {
        parts.push(format!("可能相关：{}", labels.join("、")));
    }
    if let Some(label) = intent_label(intent) {
        parts.push(format!("意图：{label}"));
    }
    parts.join("；")
}

fn format_candidate_hint(candidate: &ScopeCandidate) -> String {
    if candidate.label.is_empty() {
        return String::new();
    }
    if candidate.candidate_type != ScopeCandidateType::Dataset {
        return candidate.label.clone();
    }
    let mut details = Vec::new();
    if candidate.document_count > 0 {
        details.push(format!("{}文档", candidate.document_count));
    }
    if candidate
        .material_hints
        .iter()
        .any(|hint| hint == "audio_video")
    {
        details.push("媒体".to_string());
    }
    if details.is_empty() {
        candidate.label.clone()
    } else {
        format!("{}({})", candidate.label, details.join("/"))
    }
}

fn infer_assistant_intent(prompt: &str) -> &'static str {
    let lower_prompt = prompt.to_ascii_lowercase();
    if prompt_has_any(prompt, &lower_prompt, STATIC_PAGE_HINTS) {
        return "static_page";
    }
    if prompt_has_any(prompt, &lower_prompt, REPORT_HINTS) {
        return "report";
    }
    if prompt_is_plain_generation_without_data_need(prompt, &lower_prompt) {
        return "ordinary_chat";
    }
    if prompt_wants_video_ppt_extraction(prompt)
        || prompt_has_any(prompt, &lower_prompt, DATA_QUESTION_HINTS)
        || BUSINESS_HINTS
            .iter()
            .flat_map(|(_, hints)| hints.iter())
            .any(|hint| prompt.contains(hint))
    {
        return "data_question";
    }
    "ordinary_chat"
}

fn prompt_is_plain_generation_without_data_need(prompt: &str, lower_prompt: &str) -> bool {
    prompt_has_any(prompt, lower_prompt, PLAIN_GENERATION_HINTS)
        && !prompt_has_any(prompt, lower_prompt, EXPLICIT_DATA_NEED_HINTS)
}

fn prompt_has_any(prompt: &str, lower_prompt: &str, hints: &[&str]) -> bool {
    hints.iter().any(|hint| {
        if hint.is_ascii() {
            lower_prompt.contains(&hint.to_ascii_lowercase())
        } else {
            prompt.contains(hint)
        }
    })
}

fn prompt_has_media_detail(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_has_any(prompt, &lower_prompt, MEDIA_HINTS)
}

fn prompt_wants_video_ppt_extraction(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    prompt_has_any(prompt, &lower_prompt, VIDEO_PPT_SOURCE_HINTS)
        && prompt_has_any(prompt, &lower_prompt, VIDEO_PPT_OUTPUT_HINTS)
}

fn prompt_has_direct_video_source(prompt: &str) -> bool {
    let lower_prompt = prompt.to_ascii_lowercase();
    lower_prompt.contains("http://")
        || lower_prompt.contains("https://")
        || lower_prompt.contains(".mp4")
        || lower_prompt.contains(".mov")
        || lower_prompt.contains(".m4v")
        || lower_prompt.contains(".webm")
        || prompt.contains("公开视频")
        || prompt.contains("视频地址")
        || prompt.contains("视频链接")
}

fn intent_label(intent: &str) -> Option<&'static str> {
    match intent {
        "static_page" => Some("静态页规划"),
        "report" => Some("报表/看板"),
        "data_question" => Some("资料问答"),
        _ => None,
    }
}

fn supply_policy_for_scope(
    intent: &str,
    has_dataset: bool,
    has_memory: bool,
    detail_prompt: bool,
    video_ppt_extraction: bool,
    direct_video_source: bool,
) -> Value {
    let prefer_detail = has_dataset
        && (matches!(intent, "static_page" | "report") || detail_prompt || video_ppt_extraction);
    json!({
        "intent": intent,
        "answerPolicy": "model_authored_host_supplied",
        "actionPolicy": "model_may_request_controlled_actions_host_validates",
        "contextBudgetPolicy": if prefer_detail || has_memory {
            "quality_first_token_tolerant"
        } else {
            "compact_until_retrieval_needed"
        },
        "candidatePolicy": if has_dataset {
            "selected_or_inferred_visible_datasets_only"
        } else {
            "ordinary_chat_without_forced_dataset"
        },
        "historyPolicy": if has_memory { "intent_gated_selected" } else { "intent_gated" },
        "retrievalPolicy": if has_dataset {
            if prefer_detail { "detail_first" } else { "standard" }
        } else {
            "not_requested"
        },
        "preferDetail": prefer_detail,
        "recommendedActions": recommended_tool_actions_for_scope(intent, has_dataset, prefer_detail, detail_prompt, video_ppt_extraction, direct_video_source),
        "noFakeData": true,
    })
}

fn recommended_tool_actions_for_scope(
    intent: &str,
    has_dataset: bool,
    prefer_detail: bool,
    detail_prompt: bool,
    video_ppt_extraction: bool,
    direct_video_source: bool,
) -> Vec<&'static str> {
    let mut actions = Vec::new();
    if has_dataset {
        actions.push("retrieval.search");
    }
    if prefer_detail {
        actions.push("retrieval.read_detail");
    }
    if detail_prompt && (!video_ppt_extraction || has_dataset) {
        actions.push("media.detail");
    }
    if video_ppt_extraction {
        if direct_video_source {
            actions.push("media.resolve_video_url");
        }
        actions.push("media.extract_ppt_transcript");
    }
    match intent {
        "static_page" => actions.push("static_page.plan"),
        "report" => actions.push("report.plan"),
        _ => {}
    }
    if actions.is_empty() {
        actions.push("ordinary_chat.answer");
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::{AssistantRunCodexActionContractView, AssistantRunCodexSafetyPolicyView};
    use domain_model::{DatasetLifecycle, DatasetVisibility, TenantId};
    use std::collections::BTreeMap;

    fn dataset(title: &str, key: &str) -> Dataset {
        Dataset {
            id: DatasetId::new(),
            tenant_id: TenantId::new(),
            owner_user_id: None,
            key: key.to_string(),
            title: title.to_string(),
            description: Some(format!("{title} 默认公开数据集。")),
            lifecycle: DatasetLifecycle::Draft,
            visibility: DatasetVisibility::Public,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn codex_context_package() -> AssistantRunCodexContextPackageView {
        let mut package = AssistantRunCodexContextPackageView::new(
            domain_model::AssistantRunId::new(),
            "继续优化静态页",
        );
        package.local_thread_id = Some("local-thread-1".to_string());
        package.current_artifact = Some(json!({
            "kind": "static_page_draft",
            "draft_id": "draft-1",
            "preview_status": "stale"
        }));
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "update_static_page_module",
                "更新静态页模块",
                "只能更新当前可见静态页草稿",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "retrieve_dataset_detail",
                "检索数据集详情",
                "只能在V3选定的可见数据集范围内补充供料",
                json!({"type": "object"}),
                false,
            ),
        ];
        package.context_budget = AssistantRunCodexContextBudgetView {
            included_message_count: 2,
            selected_dataset_count: 1,
            evidence_item_count: 3,
            hidden_memory_item_count: 1,
            artifact_state_chars: 96,
            ..AssistantRunCodexContextBudgetView::default()
        };
        package.supply_quality = json!({
            "status": "partial",
            "suppliedItemCount": 3,
            "citationLocatorCount": 2,
            "fallbackChunkCount": 1,
            "mediaContextCount": 1
        });
        package.model_gateway = json!({
            "lane": "codex_conversation",
            "selected_model": {
                "mode": "provider",
                "provider": "minimax",
                "model": "MiniMax-M2.7"
            },
            "profile": {
                "profile_id": "minimax-codex-shadow",
                "provider_id": "minimax",
                "model_id": "MiniMax-M2.7",
                "wire_api": "codex_compatible_shim",
                "auth": {"configured": true},
                "capabilities": ["chat", "json", "tool_calling", "codex_compatible", "static_page"],
                "capability_flags": {
                    "chat": true,
                    "json_mode": true,
                    "tool_calling": true,
                    "static_page": true,
                    "codex_compatible": true,
                    "extra": []
                }
            },
            "safety": {
                "codex_real_execution_allowed_on_this_host": false
            }
        });
        package
    }

    fn external_action_context_package(
        prompt: &str,
        current_artifact: Option<Value>,
    ) -> AssistantRunCodexContextPackageView {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.user_prompt = prompt.to_string();
        package.current_artifact = current_artifact;
        package.context_budget.selected_dataset_count = 0;
        package.supply_quality = json!({"status": "not_requested"});
        package.startup_briefing = json!({"surface": "external_channel"});
        package.selected_scope = json!({
            "type": "external_channel",
            "channel_connection_id": "channel-third-party-1",
            "platform": "third_party",
            "conversation_external_id": "conv-1",
            "sender_external_id": "user-1",
            "message_external_id": "msg-1",
            "target_system": "third_party_artifact_api"
        });
        package.evidence_state = json!({
            "supplied_items": [{
                "retrieval_evidence_id": "evidence-1",
                "summary": "用户可见的外部资料证据"
            }]
        });
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "external_artifact.status",
                "查询第三方产物状态",
                "只读查询第三方产物状态。",
                json!({"type": "object"}),
                false,
            ),
            AssistantRunCodexActionContractView::new(
                "external_artifact.publish",
                "发布第三方产物",
                "通过 V3 受控边界发布产物。",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "external_artifact.revoke",
                "撤回第三方产物",
                "撤回已发布产物，需要确认。",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "external_business_action.invoke",
                "调用第三方事务动作",
                "跨系统业务动作，需要确认。",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "直接回答",
                json!({"type": "object"}),
                false,
            ),
        ];
        package
    }

    #[test]
    fn codex_executor_dry_run_never_invokes_or_mutates() {
        let package = codex_context_package();

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(output.status, CodexConversationExecutorStatus::ShadowDryRun);
        assert!(!output.codex_invoked);
        assert!(output.fallback_to_direct);
        assert!(output.suggested_action.is_none());
        assert!(output.assistant_message.is_none());
        assert_eq!(
            output.planned_action_types,
            vec![
                "update_static_page_module".to_string(),
                "retrieve_dataset_detail".to_string()
            ]
        );
        assert_eq!(
            output.execution_trail[0]["kind"],
            json!("codex_executor.shadow_dry_run")
        );
        assert_eq!(
            output.execution_trail[0]["supply_quality"]["status"],
            json!("partial")
        );
    }

    #[test]
    fn codex_executor_plan_only_reports_available_actions_without_invocation() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(output.status, CodexConversationExecutorStatus::PlanOnly);
        assert!(!output.codex_invoked);
        assert!(output.fallback_to_direct);
        assert_eq!(
            output.execution_trail[0]["planned_action_types"],
            json!(["update_static_page_module", "retrieve_dataset_detail"])
        );
        assert_eq!(
            output
                .suggested_action
                .as_ref()
                .and_then(|action| action.get("action_type").and_then(Value::as_str)),
            Some("update_static_page_module")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["mutation_allowed"],
            json!(false)
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["draft_id"],
            json!("draft-1")
        );
        assert_eq!(
            output.execution_trail[0]["context_budget"]["evidence_item_count"],
            json!(3)
        );
        assert_eq!(
            output.execution_trail[0]["context_budget"]["budget_pressure"],
            json!("unbounded")
        );
        assert_eq!(
            output.execution_trail[0]["context_budget"]["budget_item_count"],
            json!(0)
        );
        assert_eq!(
            output.execution_trail[0]["tool_output_policy"]["max_item_chars"],
            json!(16000)
        );
        assert_eq!(
            output.output_schema.as_ref().and_then(|schema| schema
                .pointer("/properties/suggested_action/properties/action_type/enum")
                .and_then(Value::as_array)
                .map(|items| items.len())),
            Some(2)
        );
        assert_eq!(
            output.execution_trail[0]["output_schema"]["required"],
            json!(["suggested_action"])
        );
        assert_eq!(
            output.execution_trail[0]["supply_quality"]["citationLocatorCount"],
            json!(2)
        );
        assert_eq!(
            output.execution_trail[0]["supply_quality"]["fallbackChunkCount"],
            json!(1)
        );
        assert_eq!(
            output.execution_trail[0]["model_gateway"]["lane"],
            json!("codex_conversation")
        );
        assert_eq!(
            output.execution_trail[0]["model_gateway"]["wire_api"],
            json!("codex_compatible_shim")
        );
        assert_eq!(output.model_gateway["auth_configured"], json!(true));
        assert_eq!(
            output.model_gateway["profile_id"],
            json!("minimax-codex-shadow")
        );
        assert_eq!(
            output.model_gateway["capability_manifest"]["json_mode"],
            json!(true)
        );
        assert_eq!(
            output.model_gateway["capability_manifest"]["static_page"],
            json!(true)
        );
        assert_eq!(
            output.model_gateway["codex_surface"]["codex_compatible"],
            json!(true)
        );
        assert_eq!(
            output.model_gateway["codex_surface"]["real_execution_block_reason"],
            json!("disabled_on_this_host")
        );
        assert_eq!(
            output.model_gateway["codex_real_execution_allowed"],
            json!(false)
        );
        assert_eq!(output.context_budget.selected_dataset_count, 1);
    }

    #[test]
    fn codex_executor_plan_only_blocks_preview_when_static_page_data_quality_needs_attention() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.user_prompt = "生成效果图给客户确认".to_string();
        package.selected_scope = json!({"intent": "static_page"});
        package.current_artifact = Some(json!({
            "kind": "static_page_draft",
            "draft_id": "draft-quality-1",
            "bindingQuality": {
                "attentionModules": 1,
                "modules": [
                    {
                        "moduleId": "module-risk",
                        "title": "风险趋势",
                        "bindingQualityStatus": "matched_field_candidate",
                        "chartDataFit": "needs_sample_rows",
                        "sampleRows": 0,
                        "recommendedAction": "retrieve_sample_rows"
                    }
                ]
            }
        }));
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "submit_static_page_image_preview",
                "提交静态页效果图",
                "只能提交 V3 校验后的当前草稿效果图请求",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "update_static_page_module",
                "更新静态页模块",
                "只能更新当前可见草稿",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "直接回答",
                json!({"type": "object"}),
                false,
            ),
        ];

        let output = execute_codex_conversation_plan(&package);
        let suggested_action = output
            .suggested_action
            .as_ref()
            .expect("data-quality gate should suggest module repair");

        assert_eq!(
            suggested_action["action_type"],
            json!("update_static_page_module")
        );
        assert_eq!(
            suggested_action["arguments"]["draft_id"],
            json!("draft-quality-1")
        );
        assert_eq!(
            suggested_action["arguments"]["data_quality_gate"]["has_attention"],
            json!(true)
        );
        assert_eq!(
            suggested_action["arguments"]["data_quality_gate"]["attention_modules"],
            json!(1)
        );
        assert_eq!(
            suggested_action["arguments"]["data_quality_gate"]["modules"][0]["chart_data_fit"],
            json!("needs_sample_rows")
        );
        assert_eq!(suggested_action["mutation_allowed"], json!(false));
    }

    #[test]
    fn codex_executor_plan_only_allows_preview_when_static_page_data_quality_is_confirmed() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.user_prompt = "生成效果图给客户确认".to_string();
        package.selected_scope = json!({"intent": "static_page"});
        package.current_artifact = Some(json!({
            "kind": "static_page_draft",
            "draft_id": "draft-quality-2",
            "bindingQuality": {
                "attentionModules": 0,
                "modules": [
                    {
                        "moduleId": "module-sales",
                        "title": "销售趋势",
                        "bindingQualityStatus": "confirmed",
                        "chartDataFit": "ready",
                        "sampleRows": 8,
                        "recommendedAction": "数据样本可直接驱动该模块；交付前只需确认字段口径。"
                    }
                ]
            }
        }));
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "submit_static_page_image_preview",
                "提交静态页效果图",
                "只能提交 V3 校验后的当前草稿效果图请求",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "update_static_page_module",
                "更新静态页模块",
                "只能更新当前可见草稿",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "直接回答",
                json!({"type": "object"}),
                false,
            ),
        ];

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(
            output
                .suggested_action
                .as_ref()
                .and_then(|action| action.get("action_type"))
                .and_then(Value::as_str),
            Some("submit_static_page_image_preview")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["draft_id"],
            json!("draft-quality-2")
        );
    }

    #[test]
    fn codex_executor_plan_only_suggests_safe_v3_action_contracts() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.current_artifact = None;
        package.selected_scope = json!({"intent": "static_page"});
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "create_static_page_draft",
                "创建静态页草稿",
                "只能请求 V3 创建草稿",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "直接回答",
                json!({"type": "object"}),
                false,
            ),
        ];

        let output = execute_codex_conversation_plan(&package);
        let suggested_action = output
            .suggested_action
            .as_ref()
            .expect("plan-only should suggest an action");

        assert_eq!(
            suggested_action["action_type"],
            json!("create_static_page_draft")
        );
        assert_eq!(suggested_action["requires_v3_validation"], json!(true));
        assert_eq!(suggested_action["mutation_allowed"], json!(false));
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["source"],
            json!("codex_plan_only_shadow")
        );
    }

    #[test]
    fn codex_executor_plan_only_prefers_retrieval_for_missing_selected_supply() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.current_artifact = None;
        package.selected_scope = json!({"intent": "data_question"});
        package.context_budget.selected_dataset_count = 1;
        package.supply_quality = json!({"status": "missing"});
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "retrieve_evidence",
                "检索供料证据",
                "只能请求 V3 在可见范围内检索",
                json!({"type": "object"}),
                false,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "直接回答",
                json!({"type": "object"}),
                false,
            ),
        ];

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(
            output
                .suggested_action
                .as_ref()
                .and_then(|action| action.get("action_type"))
                .and_then(Value::as_str),
            Some("retrieve_evidence")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["mutates_state"],
            json!(false)
        );
        assert_eq!(
            output
                .suggested_action
                .as_ref()
                .and_then(|action| action.pointer("/arguments/query"))
                .and_then(Value::as_str),
            Some("继续优化静态页")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["selected_scope"]["intent"],
            json!("data_question")
        );
    }

    #[test]
    fn codex_executor_plan_only_suggests_video_url_resolution() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.user_prompt = "https://example.com/talk.mp4 帮我提取视频里的PPT和原文".to_string();
        package.current_artifact = None;
        package.context_budget.selected_dataset_count = 0;
        package.supply_quality = json!({"status": "not_requested"});
        package.selected_scope = json!({
            "intent": "data_question",
            "supply_policy": {
                "recommendedActions": ["media.resolve_video_url", "media.extract_ppt_transcript"]
            }
        });
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "resolve_video_url",
                "解析公开视频地址",
                "只能请求 V3 解析直接或公开页面视频地址",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "extract_video_ppt_transcript",
                "提取视频 PPT 和原文",
                "只能在 V3 已登记视频素材后排后台解析",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "直接回答",
                json!({"type": "object"}),
                false,
            ),
        ];

        let output = execute_codex_conversation_plan(&package);
        let suggested_action = output
            .suggested_action
            .as_ref()
            .expect("plan-only should suggest a video resolver action");

        assert_eq!(suggested_action["action_type"], json!("resolve_video_url"));
        assert_eq!(suggested_action["mutates_state"], json!(true));
        assert_eq!(suggested_action["mutation_allowed"], json!(false));
        assert_eq!(
            suggested_action["arguments"]["allowed_source_types"],
            json!(["direct_video_url", "public_page_resolvable_video"])
        );
        assert_eq!(
            suggested_action["arguments"]["disallowed_source_types"],
            json!([
                "login_gated_page",
                "qr_login",
                "cookies",
                "screen_recording_bypass"
            ])
        );
    }

    #[test]
    fn codex_executor_external_artifact_status_is_read_only() {
        let package = external_action_context_package("查一下第三方产物状态", None);

        let output = execute_codex_conversation_plan(&package);
        let suggested_action = output
            .suggested_action
            .as_ref()
            .expect("external status should be suggested");

        assert_eq!(
            suggested_action["action_type"],
            json!("external_artifact.status")
        );
        assert_eq!(suggested_action["mutates_state"], json!(false));
        assert_eq!(suggested_action["requires_confirmation"], json!(false));
        assert_eq!(
            suggested_action["external_action_policy"]["risk_level"],
            json!("read_only")
        );
        assert_eq!(
            suggested_action["arguments"]["risk_level"],
            json!("read_only")
        );
        assert_eq!(
            suggested_action["arguments"]["source_evidence_refs"],
            json!(["evidence-1"])
        );
    }

    #[test]
    fn codex_executor_external_artifact_publish_is_low_risk_write() {
        let package = external_action_context_package(
            "把当前产物发布到第三方页面",
            Some(json!({
                "kind": "external_artifact",
                "id": "artifact-1"
            })),
        );

        let output = execute_codex_conversation_plan(&package);
        let suggested_action = output
            .suggested_action
            .as_ref()
            .expect("external publish should be suggested");

        assert_eq!(
            suggested_action["action_type"],
            json!("external_artifact.publish")
        );
        assert_eq!(suggested_action["mutates_state"], json!(true));
        assert_eq!(suggested_action["requires_confirmation"], json!(false));
        assert_eq!(
            suggested_action["external_action_policy"]["risk_level"],
            json!("low_risk_write")
        );
        assert_eq!(
            suggested_action["arguments"]["artifact_ref"],
            json!("artifact-1")
        );
        assert_eq!(
            suggested_action["arguments"]["arguments_redacted"]["raw_prompt_redacted"],
            json!(true)
        );
    }

    #[test]
    fn codex_executor_external_artifact_revoke_requires_high_risk_confirmation() {
        let package = external_action_context_package(
            "撤回刚才发布的第三方产物",
            Some(json!({
                "kind": "external_artifact",
                "artifact_id": "artifact-2"
            })),
        );

        let output = execute_codex_conversation_plan(&package);
        let suggested_action = output
            .suggested_action
            .as_ref()
            .expect("external revoke should be suggested");

        assert_eq!(
            suggested_action["action_type"],
            json!("external_artifact.revoke")
        );
        assert_eq!(suggested_action["requires_confirmation"], json!(true));
        assert_eq!(
            suggested_action["external_action_policy"]["risk_level"],
            json!("high_risk_write")
        );
        assert_eq!(
            suggested_action["external_action_policy"]["confirmation_mode"],
            json!("original_channel")
        );
        assert_eq!(
            suggested_action["arguments"]["artifact_ref"],
            json!("artifact-2")
        );
    }

    #[test]
    fn codex_executor_external_business_action_requires_cross_system_confirmation() {
        let package = external_action_context_package("帮用户创建一个售后工单并提交", None);

        let output = execute_codex_conversation_plan(&package);
        let suggested_action = output
            .suggested_action
            .as_ref()
            .expect("external business action should be suggested");

        assert_eq!(
            suggested_action["action_type"],
            json!("external_business_action.invoke")
        );
        assert_eq!(suggested_action["requires_confirmation"], json!(true));
        assert_eq!(
            suggested_action["external_action_policy"]["risk_level"],
            json!("cross_system")
        );
        assert_eq!(
            suggested_action["external_action_policy"]["confirmation_mode"],
            json!("trusted_customer_page")
        );
        assert_eq!(
            suggested_action["arguments"]["business_action_type"],
            json!("business_action")
        );
    }

    #[test]
    fn codex_executor_plan_only_suggests_web_search_as_read_only_v3_tool() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.user_prompt = "联网搜索一下今天这个行业的最新消息".to_string();
        package.current_artifact = None;
        package.selected_scope = json!({"intent": "ordinary_chat"});
        package.context_budget.selected_dataset_count = 0;
        package.evidence_state = json!({"status": "not_requested", "supplied_items": []});
        package.supply_quality = json!({"status": "not_requested"});
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "web_search",
                "请求外部/网页搜索",
                "只读请求 V3 搜索证据",
                json!({"type": "object"}),
                false,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "直接回答",
                json!({"type": "object"}),
                false,
            ),
        ];

        let output = execute_codex_conversation_plan(&package);
        let suggested_action = output
            .suggested_action
            .as_ref()
            .expect("web search should be suggested");

        assert_eq!(suggested_action["action_type"], json!("web_search"));
        assert_eq!(suggested_action["mutates_state"], json!(false));
        assert_eq!(suggested_action["mutation_allowed"], json!(false));
        assert_eq!(suggested_action["arguments"]["freshness"], json!("latest"));
        assert_eq!(
            suggested_action["arguments"]["evidence_contract"]
                ["must_enter_v3_search_evidence_before_citation"],
            json!(true)
        );
    }

    #[test]
    fn codex_executor_plan_only_keeps_ordinary_chat_as_final_answer() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.user_prompt = "猫为什么喜欢晒太阳？".to_string();
        package.current_artifact = None;
        package.selected_scope = json!({"intent": "ordinary_chat"});
        package.context_budget.selected_dataset_count = 0;
        package.evidence_state = json!({"status": "not_requested", "supplied_items": []});
        package.supply_quality = json!({"status": "not_requested"});
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "retrieve_evidence",
                "检索供料证据",
                "只能请求 V3 在可见范围内检索",
                json!({"type": "object"}),
                false,
            ),
            AssistantRunCodexActionContractView::new(
                "create_static_page_draft",
                "创建静态页草稿",
                "只能请求 V3 创建草稿",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "普通问答直接由模型回答",
                json!({"type": "object"}),
                false,
            ),
        ];

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(
            output
                .suggested_action
                .as_ref()
                .and_then(|action| action.get("action_type"))
                .and_then(Value::as_str),
            Some("final_answer")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["mutates_state"],
            json!(false)
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["mode"],
            json!("model_authored_answer")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_contract"]
                ["additive_context_not_capability_limit"],
            json!(true)
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_contract"]
                ["ordinary_chat_allowed_without_v3_evidence"],
            json!(true)
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_contract"]
                ["unavailable_evidence_phrase"],
            json!("当前不可见/未供料")
        );
        assert!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_contract"]
                ["external_search_rule"]
                .as_str()
                .expect("external search rule")
                .contains("Do not claim web search")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_state"]
                ["intent"],
            json!("ordinary_chat")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_state"]
                ["evidence_status"],
            json!("not_requested")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_state"]
                ["search_evidence_supplied"],
            json!(false)
        );
    }

    #[test]
    fn codex_executor_final_answer_marks_supplied_search_evidence() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.user_prompt = "基于刚才搜索结果，总结要点".to_string();
        package.current_artifact = None;
        package.selected_scope = json!({"intent": "ordinary_chat"});
        package.context_budget.selected_dataset_count = 0;
        package.evidence_state = json!({
            "status": "supplied",
            "supplied_items": [{
                "type": "web_search_result",
                "source": "web_search",
                "summary": "公开来源摘要",
                "search_evidence_id": "search-1"
            }]
        });
        package.supply_quality = json!({"status": "grounded"});
        package.available_actions = vec![AssistantRunCodexActionContractView::new(
            "final_answer",
            "模型回答",
            "普通问答直接由模型回答",
            json!({"type": "object"}),
            false,
        )];

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(
            output
                .suggested_action
                .as_ref()
                .and_then(|action| action.get("action_type"))
                .and_then(Value::as_str),
            Some("final_answer")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_state"]
                ["evidence_status"],
            json!("supplied")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_state"]
                ["supply_quality_status"],
            json!("grounded")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["v3_context_state"]
                ["search_evidence_supplied"],
            json!(true)
        );
    }

    #[test]
    fn codex_executor_plan_only_routes_html_artifact_edits_through_artifact_event() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexPlanOnly;
        package.user_prompt = "把这个 HTML 产物里的标题改成经营风险总览并应用".to_string();
        package.current_artifact = Some(json!({
            "kind": "html_artifact",
            "id": "html-static-page-handoff-1",
            "template_id": "static_page_planning_handoff",
            "interaction_mode": "action_intent"
        }));
        package.selected_scope = json!({"intent": "static_page"});
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "submit_html_artifact_event",
                "提交 HTML 产物事件",
                "只能提交 V3 校验后的 HTML artifact 事件",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "update_static_page_module",
                "更新静态页模块",
                "只能更新当前可见草稿",
                json!({"type": "object"}),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "final_answer",
                "模型回答",
                "直接回答",
                json!({"type": "object"}),
                false,
            ),
        ];

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(
            output
                .suggested_action
                .as_ref()
                .and_then(|action| action.get("action_type"))
                .and_then(Value::as_str),
            Some("submit_html_artifact_event")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["mutation_allowed"],
            json!(false)
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["artifact_id"],
            json!("html-static-page-handoff-1")
        );
        assert_eq!(
            output.execution_trail[0]["suggested_action"]["arguments"]["event_type"],
            json!("html_artifact.action_intent")
        );
    }

    #[test]
    fn codex_executor_rejects_unsafe_context_and_falls_back() {
        let mut package = codex_context_package();
        package.safety = AssistantRunCodexSafetyPolicyView {
            direct_database_access_allowed: true,
            ..AssistantRunCodexSafetyPolicyView::default()
        };

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(
            output.status,
            CodexConversationExecutorStatus::RejectedUnsafeContext
        );
        assert!(!output.codex_invoked);
        assert!(output.fallback_to_direct);
        assert!(output.planned_action_types.is_empty());
        assert_eq!(
            output.execution_trail[0]["reason"],
            json!("direct_database_access_forbidden")
        );
    }

    #[test]
    fn codex_executor_real_transport_is_unsupported_until_host_validation() {
        let mut package = codex_context_package();
        package.executor_transport = AssistantRunExecutorTransportView::CodexExecSchema;

        let output = execute_codex_conversation_plan(&package);

        assert_eq!(
            output.status,
            CodexConversationExecutorStatus::UnsupportedTransport
        );
        assert!(!output.codex_invoked);
        assert!(output.fallback_to_direct);
        assert_eq!(
            output.execution_trail[0]["transport"],
            json!("codex_exec_schema")
        );
        assert_eq!(
            output
                .host_invocation
                .as_ref()
                .and_then(|value| value.get("kind"))
                .and_then(Value::as_str),
            Some("codex_exec_output_schema")
        );
        assert_eq!(
            output.execution_trail[0]["host_invocation"]["local_execution_allowed"],
            json!(false)
        );
        assert_eq!(
            output.execution_trail[0]["host_invocation"]["command_blueprint"]["program"],
            json!("codex")
        );
        assert_eq!(
            output.execution_trail[0]["host_invocation"]["model_gateway"]["lane"],
            json!("codex_conversation")
        );
    }

    #[test]
    fn selected_dataset_stays_highest_priority() {
        let orders = dataset("订单", "orders");
        let support = dataset("客服", "support");
        let plan = plan_scope(ScopePlannerInput {
            prompt: "看看客服投诉",
            visible_datasets: &[orders.clone(), support],
            selected_dataset_id: Some(orders.id),
            conversation_memory_available: false,
        });

        assert_eq!(plan.candidates[0].id, orders.id.to_string());
        assert_eq!(plan.candidates[0].source, "user_selected");
        assert_eq!(plan.selected_scope["mode"], json!("user_selected"));
        assert_eq!(plan.intent, "data_question");
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("standard")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["preferDetail"],
            json!(false)
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["candidatePolicy"],
            json!("selected_or_inferred_visible_datasets_only")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["recommendedActions"],
            json!(["retrieval.search"])
        );
    }

    #[test]
    fn selected_dataset_can_still_include_conversation_memory() {
        let orders = dataset("订单", "orders");
        let plan = plan_scope(ScopePlannerInput {
            prompt: "继续刚才那版订单风险",
            visible_datasets: &[orders.clone()],
            selected_dataset_id: Some(orders.id),
            conversation_memory_available: true,
        });

        assert_eq!(plan.candidates[0].id, orders.id.to_string());
        assert!(plan
            .candidates
            .iter()
            .any(|candidate| candidate.candidate_type == ScopeCandidateType::ConversationMemory));
        assert_eq!(plan.selected_scope["mode"], json!("user_selected"));
        assert_eq!(
            plan.selected_scope["conversation_memory"],
            json!(["local-thread"])
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["historyPolicy"],
            json!("intent_gated_selected")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["contextBudgetPolicy"],
            json!("quality_first_token_tolerant")
        );
    }

    #[test]
    fn common_business_hint_preselects_dataset() {
        let mut orders = dataset("订单", "orders");
        orders
            .metadata
            .insert("document_count".to_string(), json!(12));
        orders
            .metadata
            .insert("estimated_word_count".to_string(), json!(3600));
        orders
            .metadata
            .insert("parse_status_summary".to_string(), json!("completed:12"));
        orders
            .metadata
            .insert("category".to_string(), json!("订单"));
        let plan = plan_scope(ScopePlannerInput {
            prompt: "总结一下销售和复购情况",
            visible_datasets: &[orders.clone()],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].id, orders.id.to_string());
        assert_eq!(plan.candidates[0].key, "orders");
        assert_eq!(plan.candidates[0].visibility, "public");
        assert_eq!(plan.candidates[0].category, "订单");
        assert_eq!(plan.candidates[0].lifecycle, "draft");
        assert_eq!(plan.candidates[0].document_count, 12);
        assert_eq!(plan.candidates[0].estimated_word_count, 3600);
        assert_eq!(plan.candidates[0].parse_status_summary, "completed:12");
        assert!(plan.hint.contains("订单(12文档)"));
        assert_eq!(plan.selected_scope["mode"], json!("preselected"));
        assert_eq!(plan.intent, "data_question");
    }

    #[test]
    fn dataset_metadata_summary_can_drive_scope_matching() {
        let mut dataset = dataset("默认公开库", "default-public");
        dataset
            .metadata
            .insert("category".to_string(), json!("订单"));
        dataset
            .metadata
            .insert("content_type_summary".to_string(), json!("spreadsheet:2"));
        dataset.metadata.insert(
            "material_hints".to_string(),
            json!(["audio_video", "transcript_possible"]),
        );

        let plan = plan_scope(ScopePlannerInput {
            prompt: "总结销售趋势，顺便看看录音有没有风险",
            visible_datasets: &[dataset.clone()],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].id, dataset.id.to_string());
        assert_eq!(plan.candidates[0].category, "订单");
        assert!(plan.candidates[0]
            .material_hints
            .contains(&"audio_video".to_string()));
        assert_eq!(plan.selected_scope["mode"], json!("preselected"));
    }

    #[test]
    fn document_title_hints_can_drive_scope_matching() {
        let mut dataset = dataset("新世界 IOA 问答测试集", "xinshijie-ioa");
        dataset.metadata.insert(
            "document_title_hints".to_string(),
            json!(["用户手册3-固定资产", "IOA系统Q&A"]),
        );

        let plan = plan_scope(ScopePlannerInput {
            prompt: "固定资产怎么操作",
            visible_datasets: &[dataset.clone()],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].id, dataset.id.to_string());
        assert_eq!(plan.candidates[0].confidence, ScopeConfidence::High);
        assert_eq!(plan.candidates[0].source, "scope_planner");
        assert_eq!(plan.selected_scope["mode"], json!("preselected"));
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("standard")
        );
    }

    #[test]
    fn document_understanding_hints_can_drive_scope_matching() {
        let mut dataset = dataset("合同资料", "contracts");
        dataset.metadata.insert(
            "noun_term_hints".to_string(),
            json!(["订单延期风险", "供应商确认"]),
        );
        dataset
            .metadata
            .insert("section_title_hints".to_string(), json!(["履约概览"]));

        let plan = plan_scope(ScopePlannerInput {
            prompt: "查找供应商确认这个主题在哪个库里",
            visible_datasets: &[dataset.clone()],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].id, dataset.id.to_string());
        assert_eq!(
            plan.candidates[0].noun_term_hints,
            vec!["订单延期风险".to_string(), "供应商确认".to_string()]
        );
        assert_eq!(
            plan.candidates[0].section_title_hints,
            vec!["履约概览".to_string()]
        );
        assert_eq!(plan.selected_scope["mode"], json!("preselected"));
    }

    #[test]
    fn media_prompt_preselects_media_dataset() {
        let media = dataset("会议录音", "meeting-media");
        let plan = plan_scope(ScopePlannerInput {
            prompt: "这段录音讲了什么，帮我提炼重点",
            visible_datasets: &[dataset("订单", "orders"), media.clone()],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].id, media.id.to_string());
        assert!(plan.hint.contains("会议录音(媒体)"));
        assert_eq!(
            plan.candidates[0].material_hints,
            vec![
                "audio_video".to_string(),
                "transcript_possible".to_string(),
                "keyframe_ocr_possible".to_string()
            ]
        );
        assert_eq!(plan.selected_scope["mode"], json!("preselected"));
        assert_eq!(plan.intent, "data_question");
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("detail_first")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["preferDetail"],
            json!(true)
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["recommendedActions"],
            json!(["retrieval.search", "retrieval.read_detail", "media.detail"])
        );
    }

    #[test]
    fn conversation_memory_is_only_candidate_when_prompt_references_history() {
        let plan = plan_scope(ScopePlannerInput {
            prompt: "继续刚才那版草稿",
            visible_datasets: &[],
            selected_dataset_id: None,
            conversation_memory_available: true,
        });

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(
            plan.candidates[0].candidate_type,
            ScopeCandidateType::ConversationMemory
        );
        assert_eq!(
            plan.selected_scope["conversation_memory"],
            json!(["local-thread"])
        );
    }

    #[test]
    fn ordinary_chat_has_no_candidates_without_hits() {
        let plan = plan_scope(ScopePlannerInput {
            prompt: "今天天气怎么样",
            visible_datasets: &[dataset("订单", "orders")],
            selected_dataset_id: None,
            conversation_memory_available: true,
        });

        assert!(plan.candidates.is_empty());
        assert_eq!(plan.selected_scope["mode"], json!("ordinary_chat"));
        assert_eq!(plan.selected_scope["conversation_memory"], json!([]));
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("not_requested")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["candidatePolicy"],
            json!("ordinary_chat_without_forced_dataset")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["recommendedActions"],
            json!(["ordinary_chat.answer"])
        );
    }

    #[test]
    fn plain_generation_with_business_terms_does_not_force_dataset_scope() {
        let plan = plan_scope(ScopePlannerInput {
            prompt: "帮我给订单客户写一句温和的欢迎语",
            visible_datasets: &[dataset("订单经营资料", "orders")],
            selected_dataset_id: None,
            conversation_memory_available: true,
        });

        assert!(plan.candidates.is_empty());
        assert_eq!(plan.intent, "ordinary_chat");
        assert_eq!(plan.selected_scope["mode"], json!("ordinary_chat"));
        assert_eq!(plan.selected_scope["datasets"], json!([]));
        assert_eq!(plan.selected_scope["conversation_memory"], json!([]));
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("not_requested")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["candidatePolicy"],
            json!("ordinary_chat_without_forced_dataset")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["recommendedActions"],
            json!(["ordinary_chat.answer"])
        );
    }

    #[test]
    fn explicit_data_need_in_generation_prompt_still_preselects_dataset() {
        let orders = dataset("订单经营资料", "orders");
        let plan = plan_scope(ScopePlannerInput {
            prompt: "基于订单数据写一句客户欢迎语",
            visible_datasets: &[orders.clone()],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].id, orders.id.to_string());
        assert_eq!(plan.intent, "data_question");
        assert_eq!(plan.selected_scope["mode"], json!("preselected"));
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("standard")
        );
    }

    #[test]
    fn direct_video_ppt_extraction_does_not_force_dataset_retrieval() {
        let plan = plan_scope(ScopePlannerInput {
            prompt: "https://example.com/talk.mp4 帮我提取视频里的PPT和原文",
            visible_datasets: &[dataset("订单", "orders"), dataset("客服", "support")],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert!(plan.candidates.is_empty());
        assert_eq!(plan.intent, "data_question");
        assert_eq!(plan.selected_scope["mode"], json!("ordinary_chat"));
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("not_requested")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["candidatePolicy"],
            json!("ordinary_chat_without_forced_dataset")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["recommendedActions"],
            json!(["media.resolve_video_url", "media.extract_ppt_transcript"])
        );
    }

    #[test]
    fn static_page_intent_prefers_detail_supply_when_dataset_matches() {
        let orders = dataset("订单", "orders");
        let plan = plan_scope(ScopePlannerInput {
            prompt: "基于订单做一页静态页经营分析",
            visible_datasets: &[orders],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.intent, "static_page");
        assert_eq!(plan.selected_scope["mode"], json!("preselected"));
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("detail_first")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["preferDetail"],
            json!(true)
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["recommendedActions"],
            json!([
                "retrieval.search",
                "retrieval.read_detail",
                "static_page.plan"
            ])
        );
        assert!(plan.hint.contains("意图：静态页规划"));
    }

    #[test]
    fn no_dataset_static_page_request_keeps_retrieval_unrequested() {
        let plan = plan_scope(ScopePlannerInput {
            prompt: "帮我先规划一页静态页",
            visible_datasets: &[],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.intent, "static_page");
        assert_eq!(plan.selected_scope["mode"], json!("ordinary_chat"));
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("not_requested")
        );
        assert_eq!(
            plan.selected_scope["supply_policy"]["recommendedActions"],
            json!(["static_page.plan"])
        );
    }
}
