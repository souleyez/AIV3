use anyhow::{anyhow, Result};
use llm_gateway::{
    render_runtime_manifest, LlmProvider, LlmRequest, MODEL_LANE_STATIC_PAGE_INTENT,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const MAX_OPERATION_COUNT: usize = 24;
const DEFAULT_MODEL: &str = "static-page-intent-v1";
const DEFAULT_CHART_RUNTIME: &str = "deterministic";
const ADVANCED_CHART_RUNTIME: &str = "echarts";
const MAX_CHART_OPTIONS_DEPTH: usize = 8;

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
    pub template_reference: Value,
    #[serde(default)]
    pub missing_evidence: Value,
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
        lane: Some(MODEL_LANE_STATIC_PAGE_INTENT.to_string()),
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
    validate_safe_text(&summary, "summary")?;
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
        "update_module" => {
            require_non_empty_string(&operation, "targetModuleId")
                .or_else(|_| require_non_empty_string(&operation, "moduleId"))?;
            validate_update_module_patch(&operation)?;
        }
        "remove_module" | "move_module" | "resize_module" | "change_data_binding" => {
            require_non_empty_string(&operation, "targetModuleId")
                .or_else(|_| require_non_empty_string(&operation, "moduleId"))?;
            if operation_type == "change_data_binding" {
                validate_data_binding(
                    operation
                        .get("dataBinding")
                        .ok_or_else(|| anyhow!("change_data_binding must include dataBinding"))?,
                )?;
            }
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
            let chart_runtime = operation
                .get("chartRuntime")
                .map(|value| validate_chart_runtime(value, "chartRuntime"))
                .transpose()?
                .flatten()
                .unwrap_or(DEFAULT_CHART_RUNTIME);
            if let Some(chart_options) = operation.get("chartOptions") {
                validate_chart_options(chart_options, chart_runtime)?;
            }
        }
        "add_module" => {
            let Some(module) = operation.get("module") else {
                return Err(anyhow!("add_module must include module"));
            };
            validate_module_contract(module)?;
        }
        "reorder_modules" => {
            if !operation.get("order").is_some_and(Value::is_array) {
                return Err(anyhow!("reorder_modules must include order array"));
            }
        }
        "refresh_summary" => {
            let summary = require_non_empty_string(&operation, "modelSummary")?;
            validate_safe_text(summary, "modelSummary")?;
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
    rename_key(object, "data_binding", "dataBinding");
    rename_key(object, "chart_options", "chartOptions");
    rename_key(object, "chart_runtime", "chartRuntime");
    rename_key(object, "runtime", "chartRuntime");
    rename_key(object, "model_summary", "modelSummary");
    rename_key(object, "queue_position", "queuePosition");
    rename_key(object, "queue_message", "queueMessage");
    rename_key(object, "preview_image", "previewImage");
    rename_key(object, "final_page", "finalPage");
    if let Some(patch) = object.get_mut("patch").and_then(Value::as_object_mut) {
        rename_key(patch, "data_label", "dataLabel");
        rename_key(patch, "data_binding", "dataBinding");
        rename_key(patch, "chart_options", "chartOptions");
        rename_key(patch, "chart_runtime", "chartRuntime");
        rename_key(patch, "runtime", "chartRuntime");
        if let Some(data_binding) = patch.get_mut("dataBinding").and_then(Value::as_object_mut) {
            normalize_data_binding_object(data_binding);
        }
        if let Some(visualization) = patch
            .get_mut("visualization")
            .and_then(Value::as_object_mut)
        {
            rename_key(visualization, "chart_options", "chartOptions");
            rename_key(visualization, "chart_runtime", "chartRuntime");
            rename_key(visualization, "runtime", "chartRuntime");
        }
    }
    if let Some(data_binding) = object.get_mut("dataBinding").and_then(Value::as_object_mut) {
        normalize_data_binding_object(data_binding);
    }
    if let Some(module) = object.get_mut("module").and_then(Value::as_object_mut) {
        rename_key(module, "data_label", "dataLabel");
        rename_key(module, "data_binding", "dataBinding");
        rename_key(module, "chart_options", "chartOptions");
        rename_key(module, "chart_runtime", "chartRuntime");
        rename_key(module, "runtime", "chartRuntime");
        if let Some(data_binding) = module.get_mut("dataBinding").and_then(Value::as_object_mut) {
            normalize_data_binding_object(data_binding);
        }
        if let Some(visualization) = module
            .get_mut("visualization")
            .and_then(Value::as_object_mut)
        {
            rename_key(visualization, "chart_options", "chartOptions");
            rename_key(visualization, "chart_runtime", "chartRuntime");
            rename_key(visualization, "runtime", "chartRuntime");
        }
    }
}

fn normalize_data_binding_object(object: &mut Map<String, Value>) {
    rename_key(object, "source_id", "sourceId");
    rename_key(object, "field_path", "fieldPath");
    rename_key(object, "evidence_ids", "evidenceIds");
}

fn validate_update_module_patch(operation: &Value) -> Result<()> {
    let patch = operation
        .get("patch")
        .ok_or_else(|| anyhow!("update_module must include patch"))?;
    let Some(object) = patch.as_object() else {
        return Err(anyhow!("update_module patch must be an object"));
    };

    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "title"
                | "content"
                | "dataLabel"
                | "dataBinding"
                | "visualization"
                | "chartOptions"
                | "chartRuntime"
                | "layout"
        ) {
            return Err(anyhow!("unsupported update_module patch field: {key}"));
        }
    }

    if let Some(title) = object.get("title") {
        validate_safe_optional_string(title, "patch.title")?;
    }
    if let Some(content) = object.get("content") {
        validate_safe_optional_string(content, "patch.content")?;
    }
    if let Some(data_label) = object.get("dataLabel") {
        validate_safe_optional_string(data_label, "patch.dataLabel")?;
    }
    if let Some(data_binding) = object.get("dataBinding") {
        validate_data_binding(data_binding)?;
    }
    if let Some(visualization) = object.get("visualization") {
        validate_visualization_patch(visualization)?;
    }
    let chart_runtime = object
        .get("chartRuntime")
        .map(|value| validate_chart_runtime(value, "patch.chartRuntime"))
        .transpose()?
        .flatten()
        .or_else(|| {
            object
                .get("visualization")
                .and_then(|visualization| visualization.get("chartRuntime"))
                .and_then(Value::as_str)
        })
        .unwrap_or(DEFAULT_CHART_RUNTIME);
    if let Some(chart_options) = object.get("chartOptions") {
        validate_chart_options(chart_options, chart_runtime)?;
    }

    Ok(())
}

fn validate_optional_string(value: &Value, field_name: &str) -> Result<()> {
    if value.is_null() || value.as_str().is_some() {
        Ok(())
    } else {
        Err(anyhow!("{field_name} must be a string when provided"))
    }
}

fn validate_safe_optional_string(value: &Value, field_name: &str) -> Result<()> {
    validate_optional_string(value, field_name)?;
    if let Some(text) = value.as_str() {
        validate_safe_text(text, field_name)?;
    }
    Ok(())
}

fn validate_safe_text(value: &str, field_name: &str) -> Result<()> {
    if static_page_text_string_is_unsafe(value) {
        Err(anyhow!("{field_name} contains unsafe text"))
    } else {
        Ok(())
    }
}

fn validate_data_binding(value: &Value) -> Result<()> {
    let Some(object) = value.as_object() else {
        return Err(anyhow!("dataBinding must be an object"));
    };
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type" | "label" | "sourceId" | "fieldPath" | "field" | "aggregation" | "evidenceIds"
        ) {
            return Err(anyhow!("unsupported dataBinding field: {key}"));
        }
    }
    for key in [
        "type",
        "label",
        "sourceId",
        "fieldPath",
        "field",
        "aggregation",
    ] {
        if let Some(value) = object.get(key) {
            validate_safe_optional_string(value, &format!("dataBinding.{key}"))?;
        }
    }
    if let Some(evidence_ids) = object.get("evidenceIds") {
        if !evidence_ids.is_array() {
            return Err(anyhow!("dataBinding.evidenceIds must be an array"));
        }
    }
    Ok(())
}

fn validate_visualization_patch(value: &Value) -> Result<()> {
    let Some(object) = value.as_object() else {
        return Err(anyhow!("visualization must be an object"));
    };
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type"
                | "label"
                | "chartRuntime"
                | "chartOptions"
                | "data"
                | "values"
                | "rows"
                | "items"
                | "sampleData"
                | "sample_data"
        ) {
            return Err(anyhow!("unsupported visualization field: {key}"));
        }
    }
    if let Some(visualization_type) = object.get("type").and_then(Value::as_str) {
        if !is_supported_visualization_type(visualization_type) {
            return Err(anyhow!(
                "unsupported static page visualization type: {visualization_type}"
            ));
        }
    }
    if let Some(label) = object.get("label") {
        validate_safe_optional_string(label, "visualization.label")?;
    }
    let chart_runtime = object
        .get("chartRuntime")
        .map(|value| validate_chart_runtime(value, "visualization.chartRuntime"))
        .transpose()?
        .flatten()
        .unwrap_or(DEFAULT_CHART_RUNTIME);
    if let Some(chart_options) = object.get("chartOptions") {
        validate_chart_options(chart_options, chart_runtime)?;
    }
    for key in [
        "data",
        "values",
        "rows",
        "items",
        "sampleData",
        "sample_data",
    ] {
        if let Some(rows) = object.get(key) {
            validate_chart_json_value(rows, 0)?;
        }
    }
    Ok(())
}

fn validate_module_contract(value: &Value) -> Result<()> {
    let Some(object) = value.as_object() else {
        return Err(anyhow!("module must be an object"));
    };
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "id" | "role"
                | "title"
                | "content"
                | "dataLabel"
                | "dataBinding"
                | "data_binding"
                | "visualization"
                | "chartOptions"
                | "chartRuntime"
                | "layout"
                | "data"
                | "values"
                | "rows"
                | "items"
                | "sampleData"
                | "sample_data"
        ) {
            return Err(anyhow!("unsupported module field: {key}"));
        }
    }
    for key in ["id", "role", "title", "content", "dataLabel"] {
        if let Some(value) = object.get(key) {
            validate_safe_optional_string(value, &format!("module.{key}"))?;
        }
    }
    if let Some(data_binding) = object
        .get("dataBinding")
        .or_else(|| object.get("data_binding"))
    {
        validate_data_binding(data_binding)?;
    }
    if let Some(visualization) = object.get("visualization") {
        validate_visualization_patch(visualization)?;
    }
    let chart_runtime = object
        .get("chartRuntime")
        .map(|value| validate_chart_runtime(value, "module.chartRuntime"))
        .transpose()?
        .flatten()
        .or_else(|| {
            object
                .get("visualization")
                .and_then(|visualization| visualization.get("chartRuntime"))
                .and_then(Value::as_str)
        })
        .unwrap_or(DEFAULT_CHART_RUNTIME);
    if let Some(chart_options) = object.get("chartOptions") {
        validate_chart_options(chart_options, chart_runtime)?;
    }
    for key in [
        "data",
        "values",
        "rows",
        "items",
        "sampleData",
        "sample_data",
    ] {
        if let Some(rows) = object.get(key) {
            validate_chart_json_value(rows, 0)?;
        }
    }
    Ok(())
}

fn validate_chart_runtime<'a>(value: &'a Value, field_name: &str) -> Result<Option<&'a str>> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(runtime) = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(anyhow!("{field_name} must be a supported chart runtime"));
    };
    if is_supported_chart_runtime(runtime) {
        Ok(Some(runtime))
    } else {
        Err(anyhow!("unsupported static page chart runtime: {runtime}"))
    }
}

fn validate_chart_options(value: &Value, chart_runtime: &str) -> Result<()> {
    let Some(object) = value.as_object() else {
        return Err(anyhow!("chartOptions must be an object"));
    };
    validate_chart_json_value(value, 0)?;
    if chart_runtime == ADVANCED_CHART_RUNTIME {
        return validate_echarts_options(object);
    }
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "showLegend"
                | "showAxis"
                | "valueFormat"
                | "dataKey"
                | "categoryKey"
                | "labelKey"
                | "valueKey"
                | "seriesKey"
                | "chartType"
        ) {
            return Err(anyhow!("unsupported chartOptions field: {key}"));
        }
    }
    Ok(())
}

fn validate_echarts_options(object: &Map<String, Value>) -> Result<()> {
    for (key, value) in object {
        if !matches!(
            key.as_str(),
            "animation"
                | "aria"
                | "backgroundColor"
                | "color"
                | "dataset"
                | "grid"
                | "legend"
                | "series"
                | "title"
                | "tooltip"
                | "xAxis"
                | "yAxis"
                | "radiusAxis"
                | "angleAxis"
                | "polar"
                | "radar"
                | "visualMap"
        ) {
            return Err(anyhow!("unsupported ECharts chartOptions field: {key}"));
        }
        if key == "series" {
            validate_echarts_series(value)?;
        }
    }
    Ok(())
}

fn validate_echarts_series(value: &Value) -> Result<()> {
    let series = match value {
        Value::Array(items) => items.as_slice(),
        Value::Object(_) => std::slice::from_ref(value),
        _ => {
            return Err(anyhow!(
                "ECharts chartOptions.series must be an object or array"
            ))
        }
    };
    for item in series {
        let Some(object) = item.as_object() else {
            return Err(anyhow!("ECharts chartOptions.series items must be objects"));
        };
        let Some(series_type) = object.get("type").and_then(Value::as_str) else {
            return Err(anyhow!("ECharts series.type is required"));
        };
        if !matches!(
            series_type,
            "bar" | "line" | "pie" | "scatter" | "gauge" | "radar" | "heatmap" | "treemap"
        ) {
            return Err(anyhow!("unsupported ECharts series type: {series_type}"));
        }
    }
    Ok(())
}

fn validate_chart_json_value(value: &Value, depth: usize) -> Result<()> {
    if depth > MAX_CHART_OPTIONS_DEPTH {
        return Err(anyhow!("chartOptions nesting is too deep"));
    }
    match value {
        Value::String(text) => {
            if chart_string_is_unsafe(text) {
                return Err(anyhow!("chartOptions contains unsafe string value"));
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_chart_json_value(item, depth + 1)?;
            }
        }
        Value::Object(object) => {
            for (key, item) in object {
                if chart_option_key_is_unsafe(key) {
                    return Err(anyhow!("chartOptions contains unsafe key: {key}"));
                }
                validate_chart_json_value(item, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn chart_option_key_is_unsafe(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "__proto__" | "prototype" | "constructor" | "renderitem"
    ) || lower.starts_with("on")
}

fn chart_string_is_unsafe(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("javascript:")
        || lower.contains("data:text/html")
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("@import")
        || lower.contains("expression(")
        || lower.contains("onerror=")
        || lower.contains("onclick=")
        || lower.contains("onload=")
        || contains_html_like_tag(&lower)
}

fn static_page_text_string_is_unsafe(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("javascript:")
        || lower.contains("data:text/html")
        || lower.contains("@import")
        || lower.contains("expression(")
        || lower.contains("onerror=")
        || lower.contains("onclick=")
        || lower.contains("onload=")
        || lower.contains("<script")
        || lower.contains("</script")
        || lower.contains("<iframe")
        || lower.contains("<style")
        || lower.contains("<link")
        || lower.contains("<img")
        || lower.contains("<svg")
        || lower.contains("<object")
        || lower.contains("<embed")
        || lower.contains("<meta")
        || lower.contains("<base")
        || lower.contains("<form")
}

fn contains_html_like_tag(value: &str) -> bool {
    let chars = value.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '<' {
            let mut next = index + 1;
            while next < chars.len() && chars[next].is_whitespace() {
                next += 1;
            }
            if next < chars.len() && chars[next] == '/' {
                next += 1;
            }
            while next < chars.len() && chars[next].is_whitespace() {
                next += 1;
            }
            if next < chars.len()
                && chars[next].is_ascii_alphabetic()
                && chars[next..].iter().any(|candidate| *candidate == '>')
            {
                return true;
            }
        }
        index += 1;
    }
    false
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
            | "reset_final_render"
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

fn is_supported_chart_runtime(chart_runtime: &str) -> bool {
    matches!(
        chart_runtime,
        DEFAULT_CHART_RUNTIME | ADVANCED_CHART_RUNTIME
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
    let binding_quality_summary =
        static_page_provider_binding_quality_summary(&request.draft_payload);
    let structure_signal_summary =
        static_page_provider_structure_signal_summary(&request.draft_payload);
    let template_reference_summary = if request.template_reference.is_null() {
        static_page_provider_template_reference_summary(&request.draft_payload)
    } else {
        static_page_provider_compact_template_reference(&request.template_reference)
    };
    let missing_evidence_summary = if request.missing_evidence.is_null() {
        static_page_provider_missing_evidence_summary(
            &request.evidence_state,
            &binding_quality_summary,
        )
    } else {
        static_page_provider_compact_missing_evidence(&request.missing_evidence)
    };
    json!({
        "instruction": [
            "You are the static page planning runtime.",
            "Return strict JSON only and follow output_contract exactly.",
            "Schema: {\"summary\":\"short Chinese summary\",\"operations\":[StaticPageDraftOperation...]}",
            "Do not answer the user directly. Do not include markdown fences or prose outside JSON.",
            "Allowed operation types: update_module, add_module, remove_module, move_module, resize_module, change_visualization, change_data_binding, reorder_modules, change_style_direction, refresh_summary, queue_image_job, update_image_job_status, mark_preview_ready, confirm_preview, reset_image_job, reset_final_render, request_final_render.",
            "For module edits prefer update_module.patch with title, content, dataBinding, visualization, chartRuntime, chartOptions, and layout.",
            "For data binding use dataBinding={type,label,sourceId,fieldPath,aggregation,evidenceIds}. For charts use visualization={type,label,chartRuntime,chartOptions}.",
            "chartRuntime must be deterministic or echarts. Use echarts only for advanced plain-JSON ECharts options; never output functions, HTML, URLs, javascript:, renderItem, or event handler keys.",
            "If assistant_context.template_reference.status is selected, use it only as style/module recipe guidance. Never copy or generate raw HTML from a template reference.",
            "Respect template providerPolicy.forbiddenOutput. Never output raw_html, remote scripts, remote CSS, provider secrets, queue credentials, or private paths.",
            "Prefer fieldPath values from draft_payload.dataSnapshot.field_candidates or draft_payload.data_snapshot.field_candidates when they exist.",
            "When assistant_context.structure_signals.sectionTitleHints exists, use those values as source structure clues for docs-page modules; preserve them as supplied headings and never invent headings.",
            "Read assistant_context.static_page_binding_quality before changing dataBinding or chart type.",
            "Do not treat a matched field candidate as renderable chart data. If chartDataFit is needs_sample_rows or missing_binding, first add/suggest module sample rows only from visible evidence, repair the binding, or switch to a non-chart visualization; if still incomplete, keep the gap visible and continue queue_image_job/request_final_render when the user asked to generate or publish.",
            "If assistant_context.missing_evidence.status is needs_evidence, keep the gap visible in module content or dataBinding, do not invent facts, and continue best-effort generation when the user asked for a page/report.",
            "Use only visible selected_scope and supplied evidence. Never invent private data."
        ],
        "output_contract": static_page_provider_output_contract(),
        "prompt": request.prompt,
        "draft_payload": request.draft_payload,
        "assistant_context": {
            "assistant_run_id": request.assistant_run_id,
            "startup_briefing": request.startup_briefing,
            "selected_scope": request.selected_scope,
            "evidence_state": request.evidence_state,
            "template_reference": template_reference_summary,
            "missing_evidence": missing_evidence_summary,
            "conversation_memory_refs": request.conversation_memory_refs,
            "messages": request.messages,
            "structure_signals": structure_signal_summary,
            "static_page_binding_quality": binding_quality_summary,
        }
    })
    .to_string()
}

fn static_page_provider_output_contract() -> Value {
    json!({
        "response": "strict_json_object",
        "required_top_level_keys": ["summary", "operations"],
        "summary": {
            "type": "string",
            "language": "zh-CN",
            "max_chars": 120,
        },
        "operations": {
            "type": "array",
            "max_items": MAX_OPERATION_COUNT,
            "allowed_types": [
                "update_module",
                "add_module",
                "remove_module",
                "move_module",
                "resize_module",
                "change_visualization",
                "change_data_binding",
                "reorder_modules",
                "change_style_direction",
                "refresh_summary",
                "queue_image_job",
                "update_image_job_status",
                "mark_preview_ready",
                "confirm_preview",
                "reset_image_job",
                "reset_final_render",
                "request_final_render"
            ],
        },
        "forbidden": [
            "raw HTML or HTML-like tags in title/content/summary",
            "remote scripts or CSS",
            "javascript: URLs",
            "provider tokens, queue credentials, private local paths",
            "unverified numbers or hidden evidence claims"
        ],
    })
}

fn static_page_provider_template_reference_summary(draft_payload: &Value) -> Value {
    draft_payload
        .get("designReferences")
        .or_else(|| draft_payload.get("design_references"))
        .and_then(first_template_reference)
        .or_else(|| {
            draft_payload
                .get("source")
                .and_then(|source| {
                    source
                        .get("templateReferences")
                        .or_else(|| source.get("template_references"))
                })
                .and_then(first_template_reference)
        })
        .map(static_page_provider_compact_template_reference)
        .or_else(|| {
            string_field(
                draft_payload,
                &["templateReferenceId", "template_reference_id"],
            )
            .map(|template_id| {
                json!({
                    "status": "id_only",
                    "templateId": template_id,
                    "policy": [
                        "template reference can guide style and module recipe only",
                        "DataMax evidence and permissions remain authoritative"
                    ],
                })
            })
        })
        .unwrap_or_else(|| {
            json!({
                "status": "none",
                "policy": [
                    "No template reference is selected; use DataMax draft and evidence only"
                ],
            })
        })
}

fn first_template_reference(value: &Value) -> Option<&Value> {
    if value.is_object() {
        return Some(value);
    }
    value
        .as_array()
        .and_then(|items| items.iter().find(|item| item.is_object()))
}

fn static_page_provider_compact_template_reference(reference: &Value) -> Value {
    let provider_policy = reference
        .get("providerPolicy")
        .or_else(|| reference.get("provider_policy"))
        .unwrap_or(&Value::Null);
    json!({
        "status": string_field(reference, &["status"]).unwrap_or_else(|| "selected".to_string()),
        "source": string_field(reference, &["source"]),
        "sourceKind": string_field(reference, &["sourceKind", "source_kind"]),
        "templateId": string_field(reference, &["templateId", "template_id", "id"]),
        "label": string_field(reference, &["label", "name"]),
        "importPolicy": string_field(reference, &["importPolicy", "import_policy"]),
        "styleDirection": string_field(reference, &["styleDirection", "style_direction"]),
        "aspectHint": string_field(reference, &["aspectHint", "aspect_hint"]),
        "designIntent": string_field(reference, &["designIntent", "design_intent"]),
        "promptHints": string_array_field(reference, &["promptHints", "prompt_hints"], 8),
        "guardrails": string_array_field(reference, &["guardrails"], 8),
        "providerPolicy": {
            "providerOutput": string_field(provider_policy, &["providerOutput", "provider_output"]),
            "forbiddenOutput": string_array_field(provider_policy, &["forbiddenOutput", "forbidden_output"], 12),
        },
        "policy": [
            "template reference controls style and module recipe only",
            "DataMax model routing, permissions, datasets, evidence, and artifacts remain authoritative",
            "provider output must be structured StaticPageDraft operations, never raw final HTML"
        ],
    })
}

fn static_page_provider_missing_evidence_summary(
    evidence_state: &Value,
    binding_quality_summary: &Value,
) -> Value {
    let status = evidence_state
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let supplied_count = number_field(evidence_state, &["supplied_count", "suppliedCount"])
        .and_then(|value| value.as_f64())
        .or_else(|| array_len_as_f64(evidence_state, &["items", "evidence", "supplied"]));
    let attention_modules = binding_quality_summary
        .get("attentionModules")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut items = Vec::new();
    if matches!(status, "not_requested" | "missing" | "empty" | "unknown")
        || supplied_count == Some(0.0)
    {
        items.push(json!({
            "code": "visible_evidence_required",
            "message": "No visible evidence was supplied to the static-page planner.",
            "recommendedAction": "retrieve_evidence",
        }));
    }
    if attention_modules > 0 {
        items.push(json!({
            "code": "chart_binding_attention_required",
            "message": "One or more chart/data modules still need sample rows, stronger binding, or a non-chart fallback.",
            "recommendedAction": "update_static_page_module",
            "moduleCount": attention_modules,
        }));
    }
    json!({
        "status": if items.is_empty() { "ready" } else { "needs_evidence" },
        "items": items,
        "policy": [
            "Do not hide missing evidence in template-assisted output",
            "Do not request final render while blocking evidence gaps remain unless the user accepts a partial draft"
        ],
    })
}

fn static_page_provider_compact_missing_evidence(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return json!({
            "status": "unknown",
            "items": [],
        });
    };
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(8)
                .map(|item| {
                    json!({
                        "code": string_field(item, &["code"]),
                        "message": string_field(item, &["message"]),
                        "recommendedAction": string_field(item, &["recommendedAction", "recommended_action"]),
                        "detailTargetCount": number_field(item, &["detailTargetCount", "detail_target_count"]),
                        "moduleCount": number_field(item, &["moduleCount", "module_count"]),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "status": string_field(value, &["status"]).unwrap_or_else(|| "unknown".to_string()),
        "items": items,
        "policy": [
            "Do not hide missing evidence in template-assisted output",
            "Do not request final render while blocking evidence gaps remain unless the user accepts a partial draft"
        ],
    })
}

fn static_page_provider_binding_quality_summary(draft_payload: &Value) -> Value {
    const MODULE_LIMIT: usize = 16;

    let module_bindings = draft_payload
        .get("dataSnapshot")
        .or_else(|| draft_payload.get("data_snapshot"))
        .and_then(|snapshot| {
            snapshot
                .get("moduleBindings")
                .or_else(|| snapshot.get("module_bindings"))
        })
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(MODULE_LIMIT)
                .map(static_page_provider_module_binding_quality)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let attention_modules = module_bindings
        .iter()
        .filter(|module| {
            module
                .get("bindingQualityStatus")
                .and_then(Value::as_str)
                .is_some_and(|status| status != "confirmed")
                || module
                    .get("chartDataFit")
                    .and_then(Value::as_str)
                    .is_some_and(|fit| matches!(fit, "needs_sample_rows" | "missing_binding"))
        })
        .count();

    json!({
        "version": 1,
        "source": "draft_payload.dataSnapshot.module_bindings",
        "moduleLimit": MODULE_LIMIT,
        "moduleCount": module_bindings.len(),
        "attentionModules": attention_modules,
        "moduleBindings": module_bindings,
        "policy": [
            "confirmed/ready modules can proceed to preview.",
            "partial inferred_signal modules are evidence-backed but not exact data; ask for extraction or confirmation before final delivery.",
            "needs_sample_rows or missing_binding chart modules should first trigger evidence expansion or binding repair; if still incomplete, publish a best-effort page with visible warnings instead of stopping."
        ],
    })
}

fn static_page_provider_structure_signal_summary(draft_payload: &Value) -> Value {
    const FIELD_LIMIT: usize = 4;
    const MODULE_LIMIT: usize = 8;
    const HINT_LIMIT: usize = 12;

    let Some(snapshot) = draft_payload
        .get("dataSnapshot")
        .or_else(|| draft_payload.get("data_snapshot"))
    else {
        return static_page_provider_empty_structure_signal_summary();
    };

    let mut section_title_hints = Vec::new();
    let field_candidates = snapshot
        .get("fieldCandidates")
        .or_else(|| snapshot.get("field_candidates"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|candidate| {
                    static_page_provider_field_path(candidate)
                        == Some("retrieval.section_title_hints")
                })
                .take(FIELD_LIMIT)
                .map(|candidate| {
                    let hints = static_page_provider_section_title_hints(candidate, HINT_LIMIT);
                    for hint in &hints {
                        push_limited_string(&mut section_title_hints, hint, HINT_LIMIT);
                    }
                    json!({
                        "sourceId": string_field(candidate, &["sourceId", "source_id"]),
                        "fieldPath": static_page_provider_field_path(candidate),
                        "label": string_field(candidate, &["label"]),
                        "kind": string_field(candidate, &["kind"]),
                        "confidence": number_field(candidate, &["confidence"]),
                        "evidenceIds": candidate
                            .get("evidenceIds")
                            .or_else(|| candidate.get("evidence_ids"))
                            .cloned()
                            .unwrap_or(Value::Null),
                        "sectionTitleHints": hints,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let bound_modules = snapshot
        .get("moduleBindings")
        .or_else(|| snapshot.get("module_bindings"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|module| {
                    let binding = module
                        .get("binding")
                        .or_else(|| module.get("dataBinding"))
                        .or_else(|| module.get("data_binding"))
                        .unwrap_or(&Value::Null);
                    let binding_quality = module
                        .get("bindingQuality")
                        .or_else(|| module.get("binding_quality"))
                        .unwrap_or(&Value::Null);
                    let matched_candidate = binding_quality
                        .get("matchedFieldCandidate")
                        .or_else(|| binding_quality.get("matched_field_candidate"))
                        .unwrap_or(&Value::Null);
                    let field_path = static_page_provider_field_path(binding)
                        .or_else(|| static_page_provider_field_path(binding_quality))
                        .or_else(|| static_page_provider_field_path(matched_candidate));
                    if field_path != Some("retrieval.section_title_hints") {
                        return None;
                    }
                    let hints =
                        static_page_provider_section_title_hints(matched_candidate, HINT_LIMIT);
                    for hint in &hints {
                        push_limited_string(&mut section_title_hints, hint, HINT_LIMIT);
                    }
                    Some(json!({
                        "moduleId": string_field(module, &["moduleId", "module_id"]),
                        "title": string_field(module, &["title"]),
                        "fieldPath": field_path,
                        "bindingQualityStatus": string_field(module, &["bindingQualityStatus", "binding_quality_status"])
                            .or_else(|| string_field(binding_quality, &["status"])),
                        "sectionTitleHints": hints,
                    }))
                })
                .take(MODULE_LIMIT)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "version": 1,
        "status": if section_title_hints.is_empty() { "none" } else { "available" },
        "source": "draft_payload.dataSnapshot.field_candidates",
        "sectionTitleHints": section_title_hints,
        "fieldCandidates": field_candidates,
        "boundModules": bound_modules,
        "policy": [
            "Section title hints are supplied source structure clues, not invented content.",
            "Use these hints to organize docs-page structure modules when relevant.",
            "Do not claim unavailable interface details just because a heading exists."
        ],
    })
}

fn static_page_provider_empty_structure_signal_summary() -> Value {
    json!({
        "version": 1,
        "status": "none",
        "sectionTitleHints": [],
        "fieldCandidates": [],
        "boundModules": [],
    })
}

fn static_page_provider_field_path(value: &Value) -> Option<&str> {
    value
        .get("fieldPath")
        .or_else(|| value.get("field_path"))
        .or_else(|| value.get("field"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn static_page_provider_section_title_hints(value: &Value, limit: usize) -> Vec<String> {
    let mut hints = Vec::new();
    for key in [
        "sectionTitleHints",
        "section_title_hints",
        "sectionTitles",
        "section_titles",
        "headingHints",
        "heading_hints",
    ] {
        if let Some(value) = value.get(key) {
            collect_limited_strings(value, &mut hints, limit);
        }
    }
    hints
}

fn static_page_provider_module_binding_quality(binding: &Value) -> Value {
    let binding_object = binding
        .get("binding")
        .or_else(|| binding.get("dataBinding"))
        .or_else(|| binding.get("data_binding"))
        .unwrap_or(&Value::Null);
    let binding_quality = binding
        .get("bindingQuality")
        .or_else(|| binding.get("binding_quality"))
        .unwrap_or(&Value::Null);
    json!({
        "moduleId": string_field(binding, &["moduleId", "module_id"]),
        "title": string_field(binding, &["title"]),
        "visualizationType": string_field(binding, &["visualizationType", "visualization_type"]),
        "chartRuntime": string_field(binding, &["chartRuntime", "chart_runtime"]),
        "dataQuality": string_field(binding, &["dataQuality", "data_quality"]),
        "bindingQualityStatus": string_field(binding, &["bindingQualityStatus", "binding_quality_status"])
            .or_else(|| string_field(binding_quality, &["status"])),
        "chartDataFit": string_field(binding, &["chartDataFit", "chart_data_fit"])
            .or_else(|| string_field(binding_quality, &["chartDataFit", "chart_data_fit"])),
        "recommendedAction": string_field(binding, &["recommendedAction", "recommended_action"])
            .or_else(|| string_field(binding_quality, &["recommendedAction", "recommended_action"])),
        "reason": string_field(binding_quality, &["reason"]),
        "sampleRows": number_field(binding_quality, &["sampleRows", "sample_rows"])
            .or_else(|| array_len_field(binding, &["sampleData", "sample_data"])),
        "sourceId": string_field(binding_object, &["sourceId", "source_id"])
            .or_else(|| string_field(binding_quality, &["sourceId", "source_id"])),
        "fieldPath": string_field(binding_object, &["fieldPath", "field_path", "field"])
            .or_else(|| string_field(binding_quality, &["fieldPath", "field_path", "field"])),
        "confidence": number_field(binding_quality, &["confidence"]),
    })
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn number_field(value: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_f64))
        .map(|number| json!(number))
}

fn array_len_field(value: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_array).map(Vec::len))
        .map(|len| json!(len))
}

fn array_len_as_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_array).map(Vec::len))
        .map(|len| len as f64)
}

fn string_array_field(value: &Value, keys: &[&str], limit: usize) -> Value {
    let items = keys
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_array))
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .take(limit)
                .map(|text| json!(text))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Value::Array(items)
}

fn collect_limited_strings(value: &Value, output: &mut Vec<String>, limit: usize) {
    if output.len() >= limit {
        return;
    }
    match value {
        Value::String(text) => push_limited_string(output, text, limit),
        Value::Array(items) => {
            for item in items {
                collect_limited_strings(item, output, limit);
                if output.len() >= limit {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn push_limited_string(output: &mut Vec<String>, text: impl AsRef<str>, limit: usize) {
    if output.len() >= limit {
        return;
    }
    let normalized = text.as_ref().trim().chars().take(80).collect::<String>();
    if !normalized.is_empty() && !output.iter().any(|existing| existing == &normalized) {
        output.push(normalized);
    }
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
    use llm_gateway::{OpenClawLlmProvider, OpenClawLlmProviderConfig, ScriptedLlmProvider};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

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
    fn provider_input_exposes_compact_binding_quality_summary() {
        let mut request = sample_request("继续优化趋势模块");
        request.draft_payload = json!({
            "modules": [{
                "id": "trend",
                "title": "趋势变化",
                "visualization": {"type": "line-chart"}
            }],
            "dataSnapshot": {
                "module_bindings": [{
                    "moduleId": "trend",
                    "title": "趋势变化",
                    "binding": {
                        "sourceId": "evidence",
                        "fieldPath": "orders.amount"
                    },
                    "visualizationType": "line-chart",
                    "chartRuntime": "deterministic",
                    "sampleData": [
                        {"label": "一月", "value": 1200},
                        {"label": "二月", "value": 1380}
                    ],
                    "dataQuality": "evidence_signal",
                    "bindingQuality": {
                        "status": "partial",
                        "reason": "matched_field_candidate_without_rows",
                        "chartDataFit": "needs_sample_rows",
                        "recommendedAction": "已匹配候选字段，但还缺少可渲染样本行。",
                        "confidence": 0.48
                    }
                }]
            }
        });

        let input = build_provider_input(&request);
        let payload = serde_json::from_str::<Value>(&input).expect("input should be JSON");
        let instructions = payload["instruction"]
            .as_array()
            .expect("provider input should include instructions");
        let quality = &payload["assistant_context"]["static_page_binding_quality"];
        let module = &quality["moduleBindings"][0];

        assert!(instructions.iter().any(|instruction| {
            instruction
                .as_str()
                .is_some_and(|text| text.contains("chartDataFit"))
        }));
        assert_eq!(quality["attentionModules"], json!(1));
        assert_eq!(module["moduleId"], json!("trend"));
        assert_eq!(module["fieldPath"], json!("orders.amount"));
        assert_eq!(module["bindingQualityStatus"], json!("partial"));
        assert_eq!(module["chartDataFit"], json!("needs_sample_rows"));
        assert_eq!(module["sampleRows"], json!(2));
        assert!(module.get("sampleData").is_none());
    }

    #[test]
    fn provider_input_exposes_template_reference_and_missing_evidence_contract() {
        let mut request = sample_request("继续优化文档页");
        request.draft_payload = json!({
            "templateReferenceId": "docs-page",
            "designReferences": [{
                "source": "html-anything",
                "sourceKind": "template_design_reference",
                "templateId": "docs-page",
                "label": "技术文档页",
                "importPolicy": "metadata_and_constraints_only",
                "styleDirection": "client-delivery",
                "designIntent": "把文档整理成清晰阅读页。",
                "promptHints": [
                    "preserve source headings",
                    "show unavailable details as missing evidence"
                ],
                "guardrails": [
                    "template reference controls style and module recipe only",
                    "provider output must become structured draft data"
                ],
                "providerPolicy": {
                    "providerOutput": "structured_static_page_draft_json",
                    "forbiddenOutput": ["raw_html", "remote_script", "private_path"]
                }
            }],
            "source": {
                "templateReferences": [{
                    "templateId": "docs-page"
                }]
            },
            "dataSnapshot": {
                "field_candidates": [{
                    "sourceId": "evidence",
                    "fieldPath": "retrieval.section_title_hints",
                    "label": "文档段落标题线索",
                    "kind": "section_titles",
                    "confidence": 0.78,
                    "evidenceIds": ["evidence-1"],
                    "sectionTitleHints": ["接口与数据", "校验与交付"]
                }],
                "module_bindings": [{
                    "moduleId": "interfaces",
                    "title": "接口与数据",
                    "binding": {
                        "sourceId": "evidence",
                        "fieldPath": "retrieval.section_title_hints"
                    },
                    "bindingQuality": {
                        "status": "confirmed",
                        "matchedFieldCandidate": {
                            "fieldPath": "retrieval.section_title_hints",
                            "sectionTitleHints": ["接口与数据", "校验与交付"]
                        }
                    }
                }]
            }
        });
        request.missing_evidence = json!({
            "status": "needs_evidence",
            "items": [{
                "code": "document_headings_or_detail_required",
                "message": "需要源文档标题或细读详情。",
                "recommended_action": "read_document_detail",
                "detail_target_count": 2
            }]
        });

        let input = build_provider_input(&request);
        let payload = serde_json::from_str::<Value>(&input).expect("input should be JSON");
        let instructions = payload["instruction"]
            .as_array()
            .expect("provider input should include instructions");
        let output_contract = &payload["output_contract"];
        let template_reference = &payload["assistant_context"]["template_reference"];
        let missing_evidence = &payload["assistant_context"]["missing_evidence"];
        let structure_signals = &payload["assistant_context"]["structure_signals"];

        assert!(instructions.iter().any(|instruction| {
            instruction
                .as_str()
                .is_some_and(|text| text.contains("style/module recipe guidance"))
        }));
        assert!(instructions.iter().any(|instruction| {
            instruction
                .as_str()
                .is_some_and(|text| text.contains("forbiddenOutput"))
        }));
        assert!(instructions.iter().any(|instruction| {
            instruction.as_str().is_some_and(|text| {
                text.contains("structure_signals.sectionTitleHints")
                    && text.contains("never invent headings")
            })
        }));
        assert_eq!(output_contract["response"], json!("strict_json_object"));
        assert_eq!(template_reference["status"], json!("selected"));
        assert_eq!(template_reference["source"], json!("html-anything"));
        assert_eq!(template_reference["templateId"], json!("docs-page"));
        assert_eq!(
            template_reference["providerPolicy"]["forbiddenOutput"][0],
            json!("raw_html")
        );
        assert_eq!(missing_evidence["status"], json!("needs_evidence"));
        assert_eq!(
            missing_evidence["items"][0]["recommendedAction"],
            json!("read_document_detail")
        );
        assert_eq!(structure_signals["status"], json!("available"));
        assert_eq!(
            structure_signals["sectionTitleHints"][0],
            json!("接口与数据")
        );
        assert_eq!(
            structure_signals["fieldCandidates"][0]["fieldPath"],
            json!("retrieval.section_title_hints")
        );
        assert_eq!(
            structure_signals["boundModules"][0]["moduleId"],
            json!("interfaces")
        );
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
    fn operation_sanitizer_rejects_raw_html_in_text_fields() {
        let unsafe_content = sanitize_static_page_operations(vec![json!({
            "type": "update_module",
            "targetModuleId": "hero",
            "patch": {
                "content": "<script>alert(1)</script>"
            }
        })]);
        let unsafe_summary = sanitize_static_page_operations(vec![json!({
            "type": "refresh_summary",
            "modelSummary": "下一版加载 javascript:alert(1)"
        })]);

        assert!(unsafe_content.is_err());
        assert!(unsafe_summary.is_err());
    }

    #[test]
    fn operation_sanitizer_accepts_stage_rollback_operations() {
        let operations = sanitize_static_page_operations(vec![
            json!({"type": "reset_image_job"}),
            json!({"type": "reset_final_render"}),
        ])
        .expect("stage rollback operations should be accepted");

        assert_eq!(operations.len(), 2);
    }

    #[test]
    fn operation_sanitizer_accepts_full_module_edit_contract() {
        let operations = sanitize_static_page_operations(vec![json!({
            "type": "update_module",
            "targetModuleId": "trend",
            "patch": {
                "title": "订单趋势",
                "content": "展示订单金额按月变化。",
                "dataBinding": {
                    "type": "selected_scope",
                    "label": "订单金额",
                    "sourceId": "selected_scope",
                    "fieldPath": "orders.amount",
                    "aggregation": "sum",
                    "evidenceIds": ["ev-1"]
                },
                "visualization": {
                    "type": "line-chart",
                    "label": "趋势折线图",
                    "chartOptions": {
                        "showLegend": true,
                        "showAxis": true,
                        "valueFormat": "currency",
                        "dataKey": "orders.amount",
                        "categoryKey": "month"
                    }
                },
                "chartOptions": {
                    "seriesKey": "segment"
                }
            }
        })])
        .expect("full edit operation should sanitize");

        assert_eq!(operations.len(), 1);
        assert_eq!(
            operations[0]["patch"]["dataBinding"]["sourceId"],
            json!("selected_scope")
        );
        assert_eq!(
            operations[0]["patch"]["visualization"]["type"],
            json!("line-chart")
        );
    }

    #[test]
    fn operation_sanitizer_rejects_unsupported_module_patch_fields() {
        let unknown_patch_field = sanitize_static_page_operations(vec![json!({
            "type": "update_module",
            "targetModuleId": "trend",
            "patch": {
                "html": "<script>alert(1)</script>"
            }
        })]);
        let invalid_chart = sanitize_static_page_operations(vec![json!({
            "type": "update_module",
            "targetModuleId": "trend",
            "patch": {
                "visualization": {
                    "type": "three-dimensional-pie"
                }
            }
        })]);

        assert!(unknown_patch_field.is_err());
        assert!(invalid_chart.is_err());
    }

    #[test]
    fn operation_sanitizer_accepts_safe_echarts_runtime_and_rejects_executable_options() {
        let operations = sanitize_static_page_operations(vec![json!({
            "type": "update_module",
            "targetModuleId": "trend",
            "patch": {
                "visualization": {
                    "type": "bar-chart",
                    "chartRuntime": "echarts",
                    "chartOptions": {
                        "tooltip": { "trigger": "axis" },
                        "xAxis": { "type": "category" },
                        "yAxis": { "type": "value" },
                        "series": [{
                            "type": "bar",
                            "name": "订单金额",
                            "data": [1200, 1380, 1510]
                        }]
                    },
                    "data": [
                        {"label": "一月", "value": 1200},
                        {"label": "二月", "value": 1380}
                    ]
                }
            }
        })])
        .expect("safe ECharts config should sanitize");

        assert_eq!(
            operations[0]["patch"]["visualization"]["chartRuntime"],
            json!("echarts")
        );
        assert_eq!(
            operations[0]["patch"]["visualization"]["chartOptions"]["series"][0]["type"],
            json!("bar")
        );
        assert_eq!(
            operations[0]["patch"]["visualization"]["data"][0]["value"],
            json!(1200)
        );

        let executable_option = sanitize_static_page_operations(vec![json!({
            "type": "update_module",
            "targetModuleId": "trend",
            "patch": {
                "visualization": {
                    "type": "bar-chart",
                    "chartRuntime": "echarts",
                    "chartOptions": {
                        "series": [{
                            "type": "custom",
                            "renderItem": "function () { return {}; }"
                        }]
                    }
                }
            }
        })]);
        let unsafe_url = sanitize_static_page_operations(vec![json!({
            "type": "change_visualization",
            "targetModuleId": "trend",
            "visualizationType": "bar-chart",
            "chartRuntime": "echarts",
            "chartOptions": {
                "series": [{
                    "type": "bar",
                    "data": ["https://example.com/track"]
                }]
            }
        })]);

        assert!(executable_option.is_err());
        assert!(unsafe_url.is_err());
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
        assert_eq!(
            outcome.runtime["llm"]["lane"],
            json!(MODEL_LANE_STATIC_PAGE_INTENT)
        );
    }

    #[test]
    fn provider_interpreter_accepts_openclaw_provider_output() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_http_request(&mut stream);
            assert!(request.contains("POST /v1/responses HTTP/1.1"));
            assert!(request.contains("\"model\":\"static-page-intent-v1\""));
            assert!(request.contains("JSON"));
            let body = json!({
                "id": "resp_static_page_openclaw",
                "output_text": json!({
                    "summary": "已根据用户要求调整模块。",
                    "operations": [{
                        "type": "update_module",
                        "targetModuleId": "kpi",
                        "patch": {
                            "title": "收入表现"
                        }
                    }]
                }).to_string()
            })
            .to_string();
            write_http_json_response(&mut stream, 200, &body);
        });

        let provider = OpenClawLlmProvider::new(
            "openclaw",
            OpenClawLlmProviderConfig {
                gateway_base_url: format!("http://{addr}"),
                token: Some("test-openclaw-token".to_string()),
                agent_id: None,
                model: None,
                model_override: None,
                prefer_responses: true,
                timeout_ms: 60_000,
            },
        )
        .expect("provider");
        let outcome = interpret_static_page_intent_with_provider(
            &sample_request("把 KPI 模块标题改成收入表现"),
            &provider,
            Some("static-page-intent-v1"),
        )
        .expect("openclaw provider output should parse");

        server.join().expect("server join");
        assert_eq!(outcome.summary, "已根据用户要求调整模块。");
        assert_eq!(outcome.operations.len(), 1);
        assert_eq!(outcome.operations[0]["type"], json!("update_module"));
        assert_eq!(outcome.runtime["provider"], json!("openclaw"));
        assert_eq!(
            outcome.runtime["llm"]["request_id"],
            json!("resp_static_page_openclaw")
        );
        assert_eq!(
            outcome.runtime["llm"]["lane"],
            json!(MODEL_LANE_STATIC_PAGE_INTENT)
        );
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut request_bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        let mut expected_len = None;
        loop {
            let read = stream.read(&mut buffer).expect("read");
            if read == 0 {
                break;
            }
            request_bytes.extend_from_slice(&buffer[..read]);
            let request = String::from_utf8_lossy(&request_bytes);
            if expected_len.is_none() {
                expected_len = request.split("\r\n").find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                });
            }
            if let Some(headers_end) = request.find("\r\n\r\n") {
                let body_len = request_bytes.len() - (headers_end + 4);
                if body_len >= expected_len.unwrap_or(0) {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&request_bytes).to_string()
    }

    fn write_http_json_response(stream: &mut TcpStream, status: u16, body: &str) {
        let reason = match status {
            200 => "OK",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "OK",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).expect("write");
    }
}
