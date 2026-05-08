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

pub fn plan_scope(input: ScopePlannerInput<'_>) -> ScopePlan {
    let prompt = input.prompt.trim();
    let mut candidates = Vec::new();
    let intent = infer_assistant_intent(prompt);

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

    if let Some(dataset) = candidates.iter().find(|candidate| {
        candidate.candidate_type == ScopeCandidateType::Dataset
            && candidate.source == "user_selected"
    }) {
        return json!({
            "mode": "user_selected",
            "datasets": [dataset.id],
            "conversation_memory": conversation_memory,
            "intent": intent,
            "supply_policy": supply_policy_for_scope(intent, true, has_memory, detail_prompt),
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
            "supply_policy": supply_policy_for_scope(intent, true, has_memory, detail_prompt),
        });
    }

    json!({
        "mode": "ordinary_chat",
        "datasets": [],
        "conversation_memory": conversation_memory,
        "intent": intent,
        "supply_policy": supply_policy_for_scope(intent, false, has_memory, detail_prompt),
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
    format!(
        "{} {} {}",
        dataset.title,
        dataset.key,
        dataset.description.clone().unwrap_or_default()
    )
}

fn dataset_material_hints(dataset: &Dataset) -> Vec<String> {
    let haystack = dataset_haystack(dataset);
    let mut hints = Vec::new();
    if MEDIA_HINTS.iter().any(|hint| haystack.contains(hint)) {
        hints.push("audio_video".to_string());
        hints.push("transcript_possible".to_string());
        hints.push("keyframe_ocr_possible".to_string());
    }
    hints
}

fn text_matches(prompt: &str, text: &str) -> bool {
    text.split(|ch: char| ch.is_whitespace() || ",，。:：/\\|_-".contains(ch))
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
    if prompt_has_any(prompt, &lower_prompt, DATA_QUESTION_HINTS)
        || BUSINESS_HINTS
            .iter()
            .flat_map(|(_, hints)| hints.iter())
            .any(|hint| prompt.contains(hint))
    {
        return "data_question";
    }
    "ordinary_chat"
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
) -> Value {
    let prefer_detail =
        has_dataset && (matches!(intent, "static_page" | "report") || detail_prompt);
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
        "recommendedActions": recommended_tool_actions_for_scope(intent, has_dataset, prefer_detail, detail_prompt),
        "noFakeData": true,
    })
}

fn recommended_tool_actions_for_scope(
    intent: &str,
    has_dataset: bool,
    prefer_detail: bool,
    detail_prompt: bool,
) -> Vec<&'static str> {
    let mut actions = Vec::new();
    if has_dataset {
        actions.push("retrieval.search");
    }
    if prefer_detail {
        actions.push("retrieval.read_detail");
    }
    if detail_prompt {
        actions.push("media.detail");
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
