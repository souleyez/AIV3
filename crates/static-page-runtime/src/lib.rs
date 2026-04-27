use anyhow::{anyhow, Result};
use llm_gateway::{render_runtime_manifest, LlmProvider, LlmRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const MAX_OPERATION_COUNT: usize = 24;
const DEFAULT_MODEL: &str = "static-page-intent-v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StaticPageIntentRequest {
    pub prompt: String,
    #[serde(default)]
    pub draft_payload: Value,
    #[serde(default)]
    pub assistant_run_id: Option<String>,
    #[serde(default)]
    pub startup_briefing: Value,
    #[serde(default)]
    pub selected_scope: Value,
    #[serde(default)]
    pub evidence_state: Value,
    #[serde(default)]
    pub conversation_memory_refs: Vec<Value>,
    #[serde(default)]
    pub messages: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaticPageIntentSource {
    Deterministic,
    Provider,
}

impl StaticPageIntentSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Provider => "provider",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StaticPageIntentOutcome {
    pub summary: String,
    pub operations: Vec<Value>,
    pub runtime: Value,
}

pub fn interpret_static_page_intent_deterministic(
    request: &StaticPageIntentRequest,
) -> Result<StaticPageIntentOutcome> {
    let interpretation = interpret_deterministic(request);
    let operations = sanitize_static_page_operations(interpretation.operations)?;
    Ok(StaticPageIntentOutcome {
        summary: interpretation.summary,
        operations,
        runtime: json!({
            "source": StaticPageIntentSource::Deterministic.as_str(),
            "mode": "deterministic",
            "model": DEFAULT_MODEL,
        }),
    })
}

pub fn interpret_static_page_intent_with_provider(
    request: &StaticPageIntentRequest,
    provider: &dyn LlmProvider,
    model: Option<&str>,
) -> Result<StaticPageIntentOutcome> {
    let model = model.unwrap_or(DEFAULT_MODEL).trim();
    let response = provider.complete(&LlmRequest {
        model: if model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            model.to_string()
        },
        system_prompt_key: None,
        input: build_provider_input(request),
    })?;
    let payload = parse_provider_payload(&response.output_text)?;
    let summary = payload
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("模型已生成静态页修改操作。")
        .to_string();
    let operations = payload
        .get("operations")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("provider response must include operations array"))?;
    let operations = sanitize_static_page_operations(operations)?;

    Ok(StaticPageIntentOutcome {
        summary,
        operations,
        runtime: json!({
            "source": StaticPageIntentSource::Provider.as_str(),
            "provider": provider.name(),
            "llm": render_runtime_manifest(&response.runtime),
        }),
    })
}

pub fn sanitize_static_page_operations(operations: Vec<Value>) -> Result<Vec<Value>> {
    if operations.len() > MAX_OPERATION_COUNT {
        return Err(anyhow!(
            "static page operation count must be {MAX_OPERATION_COUNT} or fewer"
        ));
    }

    operations
        .into_iter()
        .map(sanitize_static_page_operation)
        .collect()
}

fn sanitize_static_page_operation(mut operation: Value) -> Result<Value> {
    if contains_unsafe_key(&operation) {
        return Err(anyhow!("static page operation contains unsafe key"));
    }
    normalize_static_page_operation(&mut operation);

    let operation_type = static_page_operation_type(&operation)
        .ok_or_else(|| anyhow!("static page operation must include non-empty type"))?;
    if !is_supported_operation_type(operation_type) {
        return Err(anyhow!(
            "unsupported static page operation type: {operation_type}"
        ));
    }

    match operation_type {
        "update_module"
        | "remove_module"
        | "move_module"
        | "resize_module"
        | "change_data_binding" => {
            require_non_empty_string(&operation, "targetModuleId")
                .or_else(|_| require_non_empty_string(&operation, "moduleId"))?;
        }
        "change_style_direction" => {
            let style = require_non_empty_string(&operation, "styleDirection")?;
            if !matches!(style, "decision-brief" | "client-delivery" | "data-command") {
                return Err(anyhow!("unsupported static page style direction: {style}"));
            }
        }
        "change_visualization" => {
            require_non_empty_string(&operation, "targetModuleId")
                .or_else(|_| require_non_empty_string(&operation, "moduleId"))?;
            let visualization = require_non_empty_string(&operation, "visualizationType")?;
            if !is_supported_visualization_type(visualization) {
                return Err(anyhow!(
                    "unsupported static page visualization type: {visualization}"
                ));
            }
        }
        "add_module" => {
            if operation.get("module").is_none() {
                return Err(anyhow!("add_module must include module"));
            }
        }
        "reorder_modules" => {
            if !operation.get("order").is_some_and(Value::is_array) {
                return Err(anyhow!("reorder_modules must include order array"));
            }
        }
        "refresh_summary" => {
            require_non_empty_string(&operation, "modelSummary")?;
        }
        "update_image_job_status" => {
            require_non_empty_string(&operation, "status")?;
        }
        _ => {}
    }

    Ok(operation)
}

fn normalize_static_page_operation(operation: &mut Value) {
    let Some(object) = operation.as_object_mut() else {
        return;
    };
    rename_key(object, "target_module_id", "targetModuleId");
    rename_key(object, "module_id", "moduleId");
    rename_key(object, "style_direction", "styleDirection");
    rename_key(object, "visualization_type", "visualizationType");
    rename_key(object, "model_summary", "modelSummary");
    rename_key(object, "queue_position", "queuePosition");
    rename_key(object, "queue_message", "queueMessage");
    rename_key(object, "preview_image", "previewImage");
    rename_key(object, "final_page", "finalPage");
}

fn rename_key(object: &mut Map<String, Value>, from: &str, to: &str) {
    if object.contains_key(to) {
        return;
    }
    if let Some(value) = object.remove(from) {
        object.insert(to.to_string(), value);
    }
}

fn static_page_operation_type(operation: &Value) -> Option<&str> {
    operation
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn require_non_empty_string<'a>(operation: &'a Value, key: &str) -> Result<&'a str> {
    operation
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("operation field {key} is required"))
}

fn is_supported_operation_type(operation_type: &str) -> bool {
    matches!(
        operation_type,
        "update_module"
            | "add_module"
            | "remove_module"
            | "move_module"
            | "resize_module"
            | "change_visualization"
            | "change_data_binding"
            | "reorder_modules"
            | "change_style_direction"
            | "refresh_summary"
            | "queue_image_job"
            | "update_image_job_status"
            | "mark_preview_ready"
            | "confirm_preview"
            | "reset_image_job"
            | "request_final_render"
    )
}

fn is_supported_visualization_type(visualization_type: &str) -> bool {
    matches!(
        visualization_type,
        "headline"
            | "kpi-cards"
            | "bar-chart"
            | "line-chart"
            | "donut-chart"
            | "table"
            | "timeline"
            | "risk-matrix"
            | "text-insight"
    )
}

fn contains_unsafe_key(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            matches!(key.as_str(), "__proto__" | "constructor" | "prototype")
                || contains_unsafe_key(value)
        }),
        Value::Array(items) => items.iter().any(contains_unsafe_key),
        _ => false,
    }
}

fn build_provider_input(request: &StaticPageIntentRequest) -> String {
    json!({
        "instruction": [
            "You are the static page planning runtime.",
            "Return strict JSON only.",
            "Schema: {\"summary\":\"short Chinese summary\",\"operations\":[StaticPageDraftOperation...]}",
            "Do not answer the user directly. Do not include markdown fences.",
            "Allowed operation types: update_module, add_module, remove_module, move_module, resize_module, change_visualization, change_data_binding, reorder_modules, change_style_direction, refresh_summary, queue_image_job, update_image_job_status, mark_preview_ready, confirm_preview, reset_image_job, request_final_render.",
            "Use only visible selected_scope and supplied evidence. Never invent private data."
        ],
        "prompt": request.prompt,
        "draft_payload": request.draft_payload,
        "assistant_context": {
            "assistant_run_id": request.assistant_run_id,
            "startup_briefing": request.startup_briefing,
            "selected_scope": request.selected_scope,
            "evidence_state": request.evidence_state,
            "conversation_memory_refs": request.conversation_memory_refs,
            "messages": request.messages,
        }
    })
    .to_string()
}

fn parse_provider_payload(output_text: &str) -> Result<Value> {
    let trimmed = output_text.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }

    let stripped = strip_json_code_fence(trimmed);
    if stripped != trimmed {
        if let Ok(value) = serde_json::from_str::<Value>(&stripped) {
            return Ok(value);
        }
    }

    let start = trimmed
        .find('{')
        .ok_or_else(|| anyhow!("provider response does not contain a JSON object"))?;
    let end = trimmed
        .rfind('}')
        .ok_or_else(|| anyhow!("provider response does not contain a JSON object"))?;
    if end <= start {
        return Err(anyhow!("provider response has invalid JSON object bounds"));
    }
    serde_json::from_str::<Value>(&trimmed[start..=end])
        .map_err(|error| anyhow!("provider response is not valid JSON: {error}"))
}

fn strip_json_code_fence(text: &str) -> String {
    let mut value = text.trim();
    if let Some(stripped) = value.strip_prefix("```json") {
        value = stripped.trim();
    } else if let Some(stripped) = value.strip_prefix("```") {
        value = stripped.trim();
    }
    if let Some(stripped) = value.strip_suffix("```") {
        value = stripped.trim();
    }
    value.to_string()
}

struct DeterministicInterpretation {
    operations: Vec<Value>,
    summary: String,
}

fn interpret_deterministic(request: &StaticPageIntentRequest) -> DeterministicInterpretation {
    let prompt = request.prompt.trim();
    let normalized = prompt.to_lowercase();
    let mut operations = Vec::new();
    let mut summary_parts = Vec::new();

    if prompt_has_any(
        &normalized,
        &["老板", "高层", "董事会", "决策层", "管理层", "ceo"],
    ) {
        operations
            .push(json!({"type": "change_style_direction", "styleDirection": "decision-brief"}));
        summary_parts.push("改成高层决策简报");
    }
    if prompt_has_any(
        &normalized,
        &["客户交付", "交付报告", "售前", "方案", "客户汇报"],
    ) {
        operations
            .push(json!({"type": "change_style_direction", "styleDirection": "client-delivery"}));
        summary_parts.push("改成客户交付报告");
    }
    if prompt_has_any(
        &normalized,
        &["看板", "大屏", "运营监控", "数据密度", "指标密度"],
    ) {
        operations
            .push(json!({"type": "change_style_direction", "styleDirection": "data-command"}));
        summary_parts.push("改成数据运营看板");
    }

    if prompt_has_any(&normalized, &["风险", "隐患", "预警"])
        && prompt_has_any(
            &normalized,
            &["突出", "强调", "优先", "放前", "前面", "高亮", "重点"],
        )
    {
        operations.push(json!({
            "type": "update_module",
            "targetModuleId": "risk",
            "patch": {
                "title": "优先风险与机会",
                "content": "把客户需要优先处理的风险、影响范围和可推进机会放在更显眼的位置。",
                "layout": { "x": 0, "y": 3, "w": 7, "h": 4 }
            }
        }));
        operations.push(json!({
            "type": "reorder_modules",
            "order": module_order_with_first(&request.draft_payload, "risk"),
        }));
        summary_parts.push("把风险模块前置并放大");
    }

    if prompt_has_any(
        &normalized,
        &["减少文字", "少点字", "少一点字", "精简", "压缩", "简短"],
    ) {
        for module_id in module_ids(&request.draft_payload) {
            operations.push(json!({
                "type": "update_module",
                "targetModuleId": module_id,
                "patch": {
                    "content": "保留关键结论、数据依据和行动含义，减少解释性文字。"
                }
            }));
        }
        summary_parts.push("压缩所有模块文案");
    }

    if prompt_has_any(
        &normalized,
        &["当前数据集", "选中范围", "接数据", "数据源", "供料"],
    ) && selected_scope_has_dataset(&request.selected_scope)
    {
        let target = infer_target_module_id(&request.draft_payload, &normalized);
        operations.push(json!({
            "type": "change_data_binding",
            "targetModuleId": target,
            "dataBinding": {
                "type": "selected_scope",
                "label": "来自当前选中范围",
                "sourceId": "selected_scope"
            }
        }));
        summary_parts.push("把模块数据绑定到当前选中范围");
    }

    if prompt_has_any(&normalized, &["刚才", "之前", "上面", "前面", "继续"])
        && !request.conversation_memory_refs.is_empty()
    {
        let target = infer_target_module_id(&request.draft_payload, &normalized);
        operations.push(json!({
            "type": "change_data_binding",
            "targetModuleId": target,
            "dataBinding": {
                "type": "conversation_memory",
                "label": "来自当前对话历史",
                "sourceId": "conversation_memory"
            }
        }));
        summary_parts.push("结合当前对话历史供料");
    }

    if let Some(visualization_type) = infer_visualization_type(&normalized) {
        let target = infer_target_module_id(&request.draft_payload, &normalized);
        operations.push(json!({
            "type": "change_visualization",
            "targetModuleId": target,
            "visualizationType": visualization_type,
        }));
        summary_parts.push("调整目标模块的可视化图表");
    }

    if operations.is_empty() {
        let target = infer_target_module_id(&request.draft_payload, &normalized);
        operations.push(json!({
            "type": "update_module",
            "targetModuleId": target,
            "patch": {
                "content": format!("根据用户补充意图调整：{prompt}")
            }
        }));
        summary_parts.push("围绕目标模块应用用户修改意图");
    }

    let summary = format!("模型理解：{}。", summary_parts.join("；"));
    operations.push(json!({
        "type": "refresh_summary",
        "modelSummary": summary,
    }));

    DeterministicInterpretation {
        operations,
        summary,
    }
}

fn prompt_has_any(prompt: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| prompt.contains(keyword))
}

fn selected_scope_has_dataset(scope: &Value) -> bool {
    ["datasets", "selected"].iter().any(|key| {
        scope
            .as_object()
            .and_then(|object| object.get(*key))
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    })
}

fn module_ids(payload: &Value) -> Vec<String> {
    payload
        .as_object()
        .and_then(|object| object.get("modules"))
        .and_then(Value::as_array)
        .map(|modules| {
            modules
                .iter()
                .filter_map(|module| {
                    module
                        .as_object()
                        .and_then(|object| object.get("id"))
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect::<Vec<_>>()
        })
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| vec!["hero".to_string()])
}

fn module_order_with_first(payload: &Value, first_id: &str) -> Vec<Value> {
    let mut ordered = Vec::new();
    ordered.push(json!(first_id));
    for module_id in module_ids(payload) {
        if module_id != first_id {
            ordered.push(json!(module_id));
        }
    }
    ordered
}

fn infer_target_module_id(payload: &Value, prompt: &str) -> String {
    let module_ids = module_ids(payload);
    let by_keyword = [
        ("risk", &["风险", "机会", "隐患", "预警"][..]),
        ("trend", &["趋势", "折线", "柱状图", "走势"][..]),
        ("kpi", &["指标", "kpi", "数字"][..]),
        ("next-steps", &["建议", "下一步", "动作"][..]),
    ];
    for (module_id, keywords) in by_keyword {
        if prompt_has_any(prompt, keywords) && module_ids.iter().any(|id| id == module_id) {
            return module_id.to_string();
        }
    }
    module_ids
        .into_iter()
        .find(|id| id == "hero")
        .unwrap_or_else(|| "hero".to_string())
}

fn infer_visualization_type(prompt: &str) -> Option<&'static str> {
    let mapping = BTreeMap::from([
        ("bar-chart", vec!["柱状图", "柱图", "条形图", "对比图"]),
        ("line-chart", vec!["折线图", "趋势图", "曲线图"]),
        ("donut-chart", vec!["环图", "饼图", "占比图", "构成图"]),
        ("kpi-cards", vec!["指标卡", "kpi卡", "卡片"]),
        ("timeline", vec!["时间线", "路线图", "阶段"]),
        ("risk-matrix", vec!["风险矩阵", "优先级矩阵"]),
        ("table", vec!["表格", "明细表", "证据表"]),
    ]);
    mapping.into_iter().find_map(|(visualization, keywords)| {
        prompt_has_any(prompt, &keywords).then_some(visualization)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_gateway::ScriptedLlmProvider;

    fn sample_request(prompt: &str) -> StaticPageIntentRequest {
        StaticPageIntentRequest {
            prompt: prompt.to_string(),
            draft_payload: json!({
                "modules": [
                    {"id": "hero", "title": "核心判断"},
                    {"id": "kpi", "title": "关键指标"},
                    {"id": "trend", "title": "趋势变化"},
                    {"id": "risk", "title": "风险与机会"},
                    {"id": "next-steps", "title": "建议动作"}
                ],
                "mobileOrder": ["hero", "kpi", "trend", "risk", "next-steps"]
            }),
            selected_scope: json!({
                "datasets": [{"type": "dataset", "id": "dataset-1", "label": "订单"}]
            }),
            evidence_state: json!({"status": "supplied"}),
            ..StaticPageIntentRequest::default()
        }
    }

    #[test]
    fn deterministic_interpreter_handles_chinese_style_risk_and_chart_prompt() {
        let request = sample_request("给老板看，突出风险，把趋势换成柱状图");
        let outcome = interpret_static_page_intent_deterministic(&request).unwrap();

        assert!(outcome
            .operations
            .iter()
            .any(|operation| operation["type"] == json!("change_style_direction")));
        assert!(outcome
            .operations
            .iter()
            .any(|operation| operation["targetModuleId"] == json!("risk")));
        assert!(outcome
            .operations
            .iter()
            .any(|operation| operation["visualizationType"] == json!("bar-chart")));
        assert!(outcome.summary.contains("高层决策简报"));
    }

    #[test]
    fn deterministic_interpreter_uses_selected_scope_for_data_binding() {
        let request = sample_request("趋势模块接当前数据集的数据源");
        let outcome = interpret_static_page_intent_deterministic(&request).unwrap();

        assert!(outcome.operations.iter().any(|operation| {
            operation["type"] == json!("change_data_binding")
                && operation["dataBinding"]["sourceId"] == json!("selected_scope")
        }));
    }

    #[test]
    fn deterministic_interpreter_uses_conversation_memory_refs() {
        let mut request = sample_request("继续按刚才说的调整核心判断");
        request.conversation_memory_refs = vec![json!({"id": "memory-1"})];
        let outcome = interpret_static_page_intent_deterministic(&request).unwrap();

        assert!(outcome.operations.iter().any(|operation| {
            operation["type"] == json!("change_data_binding")
                && operation["dataBinding"]["sourceId"] == json!("conversation_memory")
        }));
    }

    #[test]
    fn operation_sanitizer_rejects_unknown_and_unsafe_operations() {
        let unknown = sanitize_static_page_operations(vec![json!({"type": "run_shell"})]);
        let unsafe_key = sanitize_static_page_operations(vec![json!({
            "type": "update_module",
            "targetModuleId": "hero",
            "patch": {"__proto__": {"polluted": true}}
        })]);

        assert!(unknown.is_err());
        assert!(unsafe_key.is_err());
    }

    #[test]
    fn provider_interpreter_accepts_strict_json_operations() {
        let provider = ScriptedLlmProvider::new("scripted").with_response_text(
            json!({
                "summary": "模型调整了图表。",
                "operations": [
                    {
                        "type": "change_visualization",
                        "targetModuleId": "trend",
                        "visualizationType": "line-chart"
                    }
                ]
            })
            .to_string(),
        );

        let outcome = interpret_static_page_intent_with_provider(
            &sample_request("换成折线图"),
            &provider,
            None,
        )
        .unwrap();

        assert_eq!(outcome.summary, "模型调整了图表。");
        assert_eq!(outcome.operations.len(), 1);
        assert_eq!(outcome.runtime["source"], json!("provider"));
    }
}
