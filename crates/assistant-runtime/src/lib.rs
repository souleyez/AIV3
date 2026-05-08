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
            candidates.push(ScopeCandidate {
                candidate_type: ScopeCandidateType::Dataset,
                id: dataset.id.to_string(),
                label: dataset_label(dataset),
                confidence: ScopeConfidence::High,
                reason: "用户当前已选中该供料范围".to_string(),
                source: "user_selected".to_string(),
                material_hints: dataset_material_hints(dataset),
            });
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
            candidates.push(ScopeCandidate {
                candidate_type: ScopeCandidateType::Dataset,
                id: dataset.id.to_string(),
                label: dataset_label(dataset),
                confidence: if matched_by_name {
                    ScopeConfidence::High
                } else {
                    ScopeConfidence::Medium
                },
                reason: if matched_by_name {
                    "用户提到数据集名称或关键字".to_string()
                } else {
                    "用户问题命中常用业务主题".to_string()
                },
                source: "scope_planner".to_string(),
                material_hints: dataset_material_hints(dataset),
            });
        }
    }

    if input.conversation_memory_available
        && CONVERSATION_HINTS.iter().any(|hint| prompt.contains(hint))
    {
        candidates.push(ScopeCandidate {
            candidate_type: ScopeCandidateType::ConversationMemory,
            id: "local-thread".to_string(),
            label: "本轮对话历史".to_string(),
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
        .map(|candidate| candidate.label.as_str())
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
        "historyPolicy": if has_memory { "intent_gated_selected" } else { "intent_gated" },
        "retrievalPolicy": if has_dataset {
            if prefer_detail { "detail_first" } else { "standard" }
        } else {
            "not_requested"
        },
        "preferDetail": prefer_detail,
        "noFakeData": true,
    })
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
    }

    #[test]
    fn common_business_hint_preselects_dataset() {
        let orders = dataset("订单", "orders");
        let plan = plan_scope(ScopePlannerInput {
            prompt: "总结一下销售和复购情况",
            visible_datasets: &[orders.clone()],
            selected_dataset_id: None,
            conversation_memory_available: false,
        });

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].id, orders.id.to_string());
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
        assert_eq!(
            plan.selected_scope["supply_policy"]["retrievalPolicy"],
            json!("not_requested")
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
    }
}
