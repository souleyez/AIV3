use serde_json::{json, Map, Value};

const ASSISTANT_RUN_REACT_ARGUMENT_MAX_BYTES: usize = 16 * 1024;
const ASSISTANT_RUN_REACT_REASON_MAX_CHARS: usize = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssistantRunReActStatus {
    Act,
    FinalAnswer,
    ReportChoice,
    AskClarifyingQuestion,
    CannotAnswer,
}

impl AssistantRunReActStatus {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim() {
            "act" => Some(Self::Act),
            "final_answer" => Some(Self::FinalAnswer),
            "report_choice" => Some(Self::ReportChoice),
            "ask_clarifying_question" => Some(Self::AskClarifyingQuestion),
            "cannot_answer" => Some(Self::CannotAnswer),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssistantRunReActActionType {
    RetrieveEvidence,
    WebSearch,
    ReadDocumentDetail,
    UpgradeParseVlm,
    RecallConversationMemory,
    ListReportOptions,
    ResolveVideoUrl,
    ExtractVideoPptTranscript,
    CreateStaticPageDraft,
    UpdateStaticPageModule,
    SubmitStaticPageImagePreview,
    RenderStaticPage,
    CreateReportDraft,
    ReportChoice,
    OpenClawMemoryRecall,
    OpenClawReadonlyExecution,
    CodexHostTask,
    FinalAnswer,
}

impl AssistantRunReActActionType {
    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value.trim() {
            "retrieve_evidence" => Some(Self::RetrieveEvidence),
            "web_search" => Some(Self::WebSearch),
            "read_document_detail" => Some(Self::ReadDocumentDetail),
            "upgrade_parse_vlm" => Some(Self::UpgradeParseVlm),
            "recall_conversation_memory" => Some(Self::RecallConversationMemory),
            "list_report_options" => Some(Self::ListReportOptions),
            "resolve_video_url" => Some(Self::ResolveVideoUrl),
            "extract_video_ppt_transcript" => Some(Self::ExtractVideoPptTranscript),
            "create_static_page_draft" => Some(Self::CreateStaticPageDraft),
            "update_static_page_module" => Some(Self::UpdateStaticPageModule),
            "submit_static_page_image_preview" => Some(Self::SubmitStaticPageImagePreview),
            "render_static_page" => Some(Self::RenderStaticPage),
            "create_report_draft" => Some(Self::CreateReportDraft),
            "report_choice" => Some(Self::ReportChoice),
            "openclaw_memory_recall" => Some(Self::OpenClawMemoryRecall),
            "openclaw_readonly_execution" => Some(Self::OpenClawReadonlyExecution),
            "codex_host_task" => Some(Self::CodexHostTask),
            "final_answer" => Some(Self::FinalAnswer),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::RetrieveEvidence => "retrieve_evidence",
            Self::WebSearch => "web_search",
            Self::ReadDocumentDetail => "read_document_detail",
            Self::UpgradeParseVlm => "upgrade_parse_vlm",
            Self::RecallConversationMemory => "recall_conversation_memory",
            Self::ListReportOptions => "list_report_options",
            Self::ResolveVideoUrl => "resolve_video_url",
            Self::ExtractVideoPptTranscript => "extract_video_ppt_transcript",
            Self::CreateStaticPageDraft => "create_static_page_draft",
            Self::UpdateStaticPageModule => "update_static_page_module",
            Self::SubmitStaticPageImagePreview => "submit_static_page_image_preview",
            Self::RenderStaticPage => "render_static_page",
            Self::CreateReportDraft => "create_report_draft",
            Self::ReportChoice => "report_choice",
            Self::OpenClawMemoryRecall => "openclaw_memory_recall",
            Self::OpenClawReadonlyExecution => "openclaw_readonly_execution",
            Self::CodexHostTask => "codex_host_task",
            Self::FinalAnswer => "final_answer",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssistantRunReActCitation {
    pub(crate) source_id: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) snippet: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AssistantRunReActDecision {
    pub(crate) status: AssistantRunReActStatus,
    pub(crate) intent: Option<String>,
    pub(crate) action_type: AssistantRunReActActionType,
    pub(crate) reason_summary: String,
    pub(crate) arguments: Value,
    pub(crate) requires_confirmation: bool,
    pub(crate) answer: Option<String>,
    pub(crate) citations: Vec<AssistantRunReActCitation>,
    pub(crate) conversation_state: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssistantRunReActParseError {
    message: String,
}

impl AssistantRunReActParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AssistantRunReActParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AssistantRunReActParseError {}

pub(crate) fn parse_assistant_run_next_action(
    output_text: &str,
) -> std::result::Result<AssistantRunReActDecision, String> {
    parse_assistant_run_react_decision(output_text).map_err(|error| error.to_string())
}

pub(crate) fn parse_assistant_run_react_decision(
    output_text: &str,
) -> std::result::Result<AssistantRunReActDecision, AssistantRunReActParseError> {
    let payload = parse_json_object_from_model_output(output_text)?;
    if contains_unsafe_json_key(&payload) {
        return Err(AssistantRunReActParseError::new(
            "payload contains unsafe key",
        ));
    }
    let object = payload.as_object().ok_or_else(|| {
        AssistantRunReActParseError::new("ReAct action payload must be a JSON object")
    })?;

    if object.contains_key("action_type") {
        return parse_legacy_decision(object);
    }

    parse_structured_decision(object)
}

fn parse_legacy_decision(
    object: &Map<String, Value>,
) -> std::result::Result<AssistantRunReActDecision, AssistantRunReActParseError> {
    let action_type = object
        .get("action_type")
        .and_then(Value::as_str)
        .and_then(AssistantRunReActActionType::from_str)
        .ok_or_else(|| AssistantRunReActParseError::new("unknown or missing action_type"))?;
    let reason_summary = normalize_reason(
        object
            .get("reason_summary")
            .and_then(Value::as_str)
            .unwrap_or(action_type.as_str()),
        action_type.as_str(),
    );
    let arguments = normalize_arguments(object.get("arguments"), None)?;
    let requires_confirmation = object
        .get("requires_confirmation")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(AssistantRunReActDecision {
        status: status_for_action(action_type),
        intent: None,
        action_type,
        reason_summary,
        answer: final_answer_from_arguments(&arguments),
        arguments,
        requires_confirmation,
        citations: parse_citations(object.get("citations")),
        conversation_state: object
            .get("conversationState")
            .or_else(|| object.get("conversation_state"))
            .cloned()
            .unwrap_or_else(|| json!({})),
    })
}

fn parse_structured_decision(
    object: &Map<String, Value>,
) -> std::result::Result<AssistantRunReActDecision, AssistantRunReActParseError> {
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .and_then(AssistantRunReActStatus::from_str)
        .ok_or_else(|| AssistantRunReActParseError::new("unknown or missing status"))?;
    let intent = object
        .get("intent")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let action_object = object.get("action").and_then(Value::as_object);
    let fallback_answer = object
        .get("answer")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let action_type = match status {
        AssistantRunReActStatus::FinalAnswer
        | AssistantRunReActStatus::AskClarifyingQuestion
        | AssistantRunReActStatus::CannotAnswer => action_object
            .and_then(|action| action.get("type"))
            .and_then(Value::as_str)
            .and_then(AssistantRunReActActionType::from_str)
            .unwrap_or(AssistantRunReActActionType::FinalAnswer),
        AssistantRunReActStatus::ReportChoice => action_object
            .and_then(|action| action.get("type"))
            .and_then(Value::as_str)
            .and_then(AssistantRunReActActionType::from_str)
            .unwrap_or(AssistantRunReActActionType::ReportChoice),
        AssistantRunReActStatus::Act => action_object
            .and_then(|action| action.get("type"))
            .and_then(Value::as_str)
            .and_then(AssistantRunReActActionType::from_str)
            .ok_or_else(|| AssistantRunReActParseError::new("unknown or missing action.type"))?,
    };
    let reason_summary = normalize_reason(
        object
            .get("reason")
            .or_else(|| object.get("reason_summary"))
            .and_then(Value::as_str)
            .unwrap_or(action_type.as_str()),
        action_type.as_str(),
    );
    let mut arguments = normalize_arguments(
        action_object.and_then(|action| action.get("arguments")),
        fallback_answer,
    )?;
    if status == AssistantRunReActStatus::ReportChoice {
        if let Some(choice) = object
            .get("choice")
            .or_else(|| object.get("report_choice"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Some(arguments_object) = arguments.as_object_mut() {
                arguments_object
                    .entry("choice".to_string())
                    .or_insert_with(|| json!(choice));
            }
        }
    }
    let requires_confirmation = object
        .get("requires_confirmation")
        .or_else(|| object.get("requiresConfirmation"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(AssistantRunReActDecision {
        status,
        intent,
        action_type,
        reason_summary,
        answer: fallback_answer
            .map(ToOwned::to_owned)
            .or_else(|| final_answer_from_arguments(&arguments)),
        arguments,
        requires_confirmation,
        citations: parse_citations(object.get("citations")),
        conversation_state: object
            .get("conversationState")
            .or_else(|| object.get("conversation_state"))
            .cloned()
            .unwrap_or_else(|| json!({})),
    })
}

fn normalize_arguments(
    raw_arguments: Option<&Value>,
    fallback_answer: Option<&str>,
) -> std::result::Result<Value, AssistantRunReActParseError> {
    let mut arguments = raw_arguments.cloned().unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Err(AssistantRunReActParseError::new(
            "arguments must be a JSON object",
        ));
    }
    if let Some(answer) = fallback_answer {
        if arguments.get("content").and_then(Value::as_str).is_none() {
            if let Some(object) = arguments.as_object_mut() {
                object.insert("content".to_string(), json!(answer));
            }
        }
    }
    let argument_bytes = serde_json::to_vec(&arguments)
        .map_err(|error| {
            AssistantRunReActParseError::new(format!(
                "arguments must be serializable JSON: {error}"
            ))
        })?
        .len();
    if argument_bytes > ASSISTANT_RUN_REACT_ARGUMENT_MAX_BYTES {
        return Err(AssistantRunReActParseError::new(format!(
            "arguments exceed {} bytes",
            ASSISTANT_RUN_REACT_ARGUMENT_MAX_BYTES
        )));
    }
    Ok(arguments)
}

fn normalize_reason(raw_reason: &str, fallback: &str) -> String {
    let reason = raw_reason.trim();
    let reason = if reason.is_empty() { fallback } else { reason };
    reason
        .chars()
        .take(ASSISTANT_RUN_REACT_REASON_MAX_CHARS)
        .collect()
}

fn status_for_action(action_type: AssistantRunReActActionType) -> AssistantRunReActStatus {
    match action_type {
        AssistantRunReActActionType::FinalAnswer => AssistantRunReActStatus::FinalAnswer,
        AssistantRunReActActionType::ReportChoice => AssistantRunReActStatus::ReportChoice,
        _ => AssistantRunReActStatus::Act,
    }
}

fn final_answer_from_arguments(arguments: &Value) -> Option<String> {
    arguments
        .get("content")
        .or_else(|| arguments.get("answer"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_citations(raw_citations: Option<&Value>) -> Vec<AssistantRunReActCitation> {
    raw_citations
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let object = item.as_object()?;
                    Some(AssistantRunReActCitation {
                        source_id: object
                            .get("source_id")
                            .or_else(|| object.get("sourceId"))
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        title: object
                            .get("title")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        snippet: object
                            .get("snippet")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_json_object_from_model_output(
    output_text: &str,
) -> std::result::Result<Value, AssistantRunReActParseError> {
    let trimmed = output_text.trim();
    if trimmed.is_empty() {
        return Err(AssistantRunReActParseError::new("model output is empty"));
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    let start = trimmed.find('{').ok_or_else(|| {
        AssistantRunReActParseError::new("model output does not contain a JSON object")
    })?;
    let end = trimmed.rfind('}').ok_or_else(|| {
        AssistantRunReActParseError::new("model output does not contain a complete JSON object")
    })?;
    if end <= start {
        return Err(AssistantRunReActParseError::new(
            "model output has invalid JSON object bounds",
        ));
    }
    serde_json::from_str::<Value>(&trimmed[start..=end]).map_err(|error| {
        AssistantRunReActParseError::new(format!("model output JSON is invalid: {error}"))
    })
}

fn contains_unsafe_json_key(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            matches!(key.as_str(), "__proto__" | "constructor" | "prototype")
                || contains_unsafe_json_key(value)
        }),
        Value::Array(items) => items.iter().any(contains_unsafe_json_key),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_v3_shape() {
        let decision = parse_assistant_run_react_decision(
            r#"{"action_type":"retrieve_evidence","reason_summary":"检索资料","arguments":{"query":"订单风险"},"requires_confirmation":false}"#,
        )
        .expect("current shape should parse");

        assert_eq!(decision.status, AssistantRunReActStatus::Act);
        assert_eq!(
            decision.action_type,
            AssistantRunReActActionType::RetrieveEvidence
        );
        assert_eq!(decision.reason_summary, "检索资料");
        assert_eq!(decision.arguments["query"], json!("订单风险"));
    }

    #[test]
    fn parses_java_reference_shape() {
        let decision = parse_assistant_run_react_decision(
            r#"{"status":"act","intent":"question","reason":"检索资料","action":{"type":"retrieve_evidence","arguments":{"query":"订单风险"}}}"#,
        )
        .expect("structured shape should parse");

        assert_eq!(decision.status, AssistantRunReActStatus::Act);
        assert_eq!(decision.intent.as_deref(), Some("question"));
        assert_eq!(
            decision.action_type,
            AssistantRunReActActionType::RetrieveEvidence
        );
        assert_eq!(decision.arguments["query"], json!("订单风险"));
    }

    #[test]
    fn parses_web_search_action() {
        let decision = parse_assistant_run_react_decision(
            r#"{"status":"act","intent":"current_info","reason":"需要外部搜索证据","action":{"type":"web_search","arguments":{"query":"V3 最新发布状态","reason":"用户询问最新情况","freshness":"latest"}}}"#,
        )
        .expect("web search action should parse");

        assert_eq!(decision.status, AssistantRunReActStatus::Act);
        assert_eq!(decision.action_type, AssistantRunReActActionType::WebSearch);
        assert_eq!(decision.arguments["query"], json!("V3 最新发布状态"));
        assert_eq!(decision.arguments["freshness"], json!("latest"));
    }

    #[test]
    fn parses_upgrade_parse_vlm_action() {
        let decision = parse_assistant_run_react_decision(
            r#"{"status":"act","intent":"data_question","reason":"解析质量不足，需要受控升级","action":{"type":"upgrade_parse_vlm","arguments":{"document_id":"00000000-0000-0000-0000-000000000001","page_hint":[1],"question_focus":"邓工是谁"}}}"#,
        )
        .expect("VLM parse upgrade action should parse");

        assert_eq!(
            AssistantRunReActActionType::UpgradeParseVlm.as_str(),
            "upgrade_parse_vlm"
        );
        assert_eq!(
            decision.action_type,
            AssistantRunReActActionType::UpgradeParseVlm
        );
        assert_eq!(decision.arguments["page_hint"][0], json!(1));
    }

    #[test]
    fn parses_terminal_answer_from_arguments_or_top_level_answer() {
        let legacy = parse_assistant_run_react_decision(
            r#"{"action_type":"final_answer","reason_summary":"完成","arguments":{"content":"可以回答"}}"#,
        )
        .expect("legacy final answer should parse");
        assert_eq!(legacy.answer.as_deref(), Some("可以回答"));
        assert_eq!(legacy.arguments["content"], json!("可以回答"));

        let structured = parse_assistant_run_react_decision(
            r#"{"status":"final_answer","intent":"question","reason":"完成","answer":"结构化回答"}"#,
        )
        .expect("structured final answer should parse");
        assert_eq!(structured.answer.as_deref(), Some("结构化回答"));
        assert_eq!(structured.arguments["content"], json!("结构化回答"));
        assert_eq!(
            structured.action_type,
            AssistantRunReActActionType::FinalAnswer
        );
    }

    #[test]
    fn parses_report_choice_terminal_without_action_object() {
        let decision = parse_assistant_run_react_decision(
            r#"{"status":"report_choice","intent":"report","reason":"用户选择生成报表","choice":"create_report"}"#,
        )
        .expect("report choice should parse as terminal status");

        assert_eq!(decision.status, AssistantRunReActStatus::ReportChoice);
        assert_eq!(
            decision.action_type,
            AssistantRunReActActionType::ReportChoice
        );
        assert_eq!(decision.arguments["choice"], json!("create_report"));
    }

    #[test]
    fn rejects_unknown_status_and_unknown_action() {
        let status_error = parse_assistant_run_react_decision(
            r#"{"status":"thinking","intent":"question","reason":"x","action":{"type":"retrieve_evidence","arguments":{}}}"#,
        )
        .expect_err("unknown status should fail");
        assert!(status_error.to_string().contains("status"));

        let action_error = parse_assistant_run_react_decision(
            r#"{"status":"act","intent":"question","reason":"x","action":{"type":"run_shell","arguments":{}}}"#,
        )
        .expect_err("unknown action should fail");
        assert!(action_error.to_string().contains("action"));
    }

    #[test]
    fn parses_codex_host_task_action() {
        let decision = parse_assistant_run_react_decision(
            r#"{"status":"act","intent":"task","reason":"需要宿主执行","action":{"type":"codex_host_task","arguments":{"capability":"inspect_project"}}}"#,
        )
        .expect("codex host task should parse");

        assert_eq!(decision.status, AssistantRunReActStatus::Act);
        assert_eq!(
            decision.action_type,
            AssistantRunReActActionType::CodexHostTask
        );
        assert_eq!(decision.arguments["capability"], json!("inspect_project"));
    }

    #[test]
    fn parses_video_ppt_actions() {
        let resolve = parse_assistant_run_react_decision(
            r#"{"status":"act","intent":"data_question","reason":"解析视频地址","action":{"type":"resolve_video_url","arguments":{"source_url":"https://example.com/talk.mp4"}}}"#,
        )
        .expect("video resolver action should parse");
        let extract = parse_assistant_run_react_decision(
            r#"{"action_type":"extract_video_ppt_transcript","reason_summary":"提取视频 PPT","arguments":{"asset_id":"asset-1"},"requires_confirmation":false}"#,
        )
        .expect("video extraction action should parse");

        assert_eq!(
            resolve.action_type,
            AssistantRunReActActionType::ResolveVideoUrl
        );
        assert_eq!(
            resolve.arguments["source_url"],
            json!("https://example.com/talk.mp4")
        );
        assert_eq!(
            extract.action_type,
            AssistantRunReActActionType::ExtractVideoPptTranscript
        );
        assert_eq!(extract.arguments["asset_id"], json!("asset-1"));
    }

    #[test]
    fn rejects_unsafe_keys_anywhere_in_payload() {
        let error = parse_assistant_run_react_decision(
            r#"{"status":"act","intent":"question","reason":"x","action":{"type":"retrieve_evidence","arguments":{"__proto__":{"polluted":true}}}}"#,
        )
        .expect_err("unsafe keys should fail");
        assert!(error.to_string().contains("unsafe"));

        let top_level_error = parse_assistant_run_react_decision(
            r#"{"status":"act","constructor":{},"action":{"type":"retrieve_evidence","arguments":{}}}"#,
        )
        .expect_err("top-level unsafe keys should fail");
        assert!(top_level_error.to_string().contains("unsafe"));
    }

    #[test]
    fn truncates_oversized_reason_summary() {
        let long_reason = "长".repeat(260);
        let payload = json!({
            "status": "act",
            "intent": "question",
            "reason": long_reason,
            "action": {
                "type": "retrieve_evidence",
                "arguments": {"query": "订单"}
            }
        });
        let decision = parse_assistant_run_react_decision(&payload.to_string())
            .expect("long reason should be truncated, not rejected");

        assert_eq!(
            decision.reason_summary.chars().count(),
            ASSISTANT_RUN_REACT_REASON_MAX_CHARS
        );
    }
}
