use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const STATIC_PAGE_RENDERER_ID: &str = "static-page-renderer-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticPageRenderRequest {
    pub draft_id: String,
    pub assistant_run_id: String,
    pub title: String,
    #[serde(default)]
    pub draft_payload: Value,
    #[serde(default)]
    pub selected_scope: Value,
    #[serde(default)]
    pub visibility_snapshot: Value,
    #[serde(default)]
    pub preview_asset_key: Option<String>,
    #[serde(default)]
    pub image_job_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticPageRenderResult {
    pub html: String,
    pub asset_manifest: Value,
}

pub fn render_static_page(request: &StaticPageRenderRequest) -> StaticPageRenderResult {
    let modules = static_page_payload_modules(&request.draft_payload);
    let style = static_page_payload_string(
        &request.draft_payload,
        &["styleDirection", "style_direction"],
    )
    .unwrap_or_else(|| "client-delivery".to_string());
    let visual_spec =
        static_page_payload_value(&request.draft_payload, &["visualSpec", "visual_spec"])
            .unwrap_or_else(|| fallback_visual_spec(&style));
    let render_spec =
        static_page_payload_value(&request.draft_payload, &["renderSpec", "render_spec"])
            .unwrap_or_else(fallback_render_spec);
    let data_snapshot =
        static_page_payload_value(&request.draft_payload, &["dataSnapshot", "data_snapshot"])
            .unwrap_or_else(|| json!({"source": "static-page-renderer-fallback"}));
    let preview_contract = static_page_payload_value(
        &request.draft_payload,
        &["previewContract", "preview_contract"],
    )
    .unwrap_or_else(|| json!({"status": "unknown"}));
    let preview = request.preview_asset_key.as_deref().unwrap_or("no-preview");
    let visual_style_attr = static_page_visual_style_attr(&visual_spec);
    let mobile_order =
        static_page_payload_string_array(&request.draft_payload, &["mobileOrder", "mobile_order"]);
    let module_html = modules
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, module)| render_module_html(module, index, &mobile_order, &data_snapshot))
        .collect::<Vec<_>>()
        .join("\n");
    let chart_runtime_manifest = build_chart_runtime_manifest(&modules, &data_snapshot);
    let echarts_hydration_script = render_echarts_hydration_script(&modules, &data_snapshot);
    let html = format!(
        concat!(
            "<!doctype html><html><head><meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
            "<title>{}</title><style>{}</style></head>",
            "<body class=\"static-page style-{}\" data-preview=\"{}\" style=\"{}\">",
            "<main><header class=\"cover\"><span>{}</span><h1>{}</h1><p>{}</p></header>",
            "<section class=\"module-grid\" aria-label=\"静态页模块\">{}</section></main>{}",
            "</body></html>"
        ),
        escape_html(&request.title),
        STATIC_PAGE_RENDER_CSS,
        escape_html(&style),
        escape_html(preview),
        escape_html(&visual_style_attr),
        escape_html(style_label(&style)),
        escape_html(&request.title),
        escape_html(
            &static_page_payload_string(&request.draft_payload, &["modelSummary", "model_summary"])
                .unwrap_or_else(|| "按确认效果图和模块规划生成静态页。".to_string()),
        ),
        module_html,
        echarts_hydration_script,
    );
    let export_package = build_export_package_manifest(
        &request.draft_id,
        &modules,
        &data_snapshot,
        &chart_runtime_manifest,
        request.preview_asset_key.as_deref(),
    );
    let asset_manifest = json!({
        "draft_id": request.draft_id,
        "assistant_run_id": request.assistant_run_id,
        "style_direction": style,
        "preview_asset_key": request.preview_asset_key,
        "image_job_id": request.image_job_id,
        "design_contract": {
            "source": "StaticPageDraft",
            "preview_role": "visual_contract",
            "final_role": "html_css_svg_renderer",
            "advanced_chart_role": "echarts_json_hydration",
        },
        "visual_spec": visual_spec,
        "render_spec": render_spec,
        "data_snapshot": data_snapshot,
        "chart_runtime": chart_runtime_manifest,
        "export_package": export_package,
        "preview_contract": preview_contract,
        "module_count": modules.as_array().map(Vec::len).unwrap_or(0),
        "modules": modules,
        "selected_scope": request.selected_scope,
        "visibility_snapshot": request.visibility_snapshot,
        "renderer": STATIC_PAGE_RENDERER_ID,
    });

    StaticPageRenderResult {
        html,
        asset_manifest,
    }
}

fn build_export_package_manifest(
    draft_id: &str,
    modules: &Value,
    data_snapshot: &Value,
    chart_runtime_manifest: &Value,
    preview_asset_key: Option<&str>,
) -> Value {
    let module_count = modules.as_array().map(Vec::len).unwrap_or(0);
    let missing_or_partial_modules = chart_runtime_manifest
        .get("modules")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|module| {
                    module
                        .get("dataQuality")
                        .and_then(Value::as_str)
                        .map(|quality| quality != "complete")
                        .unwrap_or(true)
                })
                .count()
        })
        .unwrap_or(module_count);
    let assets = preview_asset_key
        .map(|asset_key| {
            json!([{
                "path": asset_key,
                "role": "confirmed_effect_preview"
            }])
        })
        .unwrap_or_else(|| json!([]));
    let data_snapshot_source = data_snapshot
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    json!({
        "kind": "static-page-export-package",
        "version": 1,
        "status": "rendered",
        "draft_id": draft_id,
        "files": [
            {
                "path": "index.html",
                "role": "rendered_static_page",
                "mime": "text/html"
            },
            {
                "path": "asset-manifest.json",
                "role": "renderer_manifest",
                "mime": "application/json"
            },
            {
                "path": "data-snapshot.json",
                "role": "render_data_snapshot",
                "mime": "application/json"
            },
            {
                "path": "modules.json",
                "role": "editable_module_plan",
                "mime": "application/json"
            }
        ],
        "assets": assets,
        "runtime_requirements": [{
            "name": "Apache ECharts",
            "package": "echarts",
            "license": "Apache-2.0",
            "required": false,
            "role": "optional_advanced_chart_hydration",
            "note": "index.html keeps deterministic DOM/SVG chart fallback and does not inject remote scripts; approved hosts can provide ECharts to hydrate safe JSON option islands."
        }],
        "debug": {
            "renderer": STATIC_PAGE_RENDERER_ID,
            "module_count": module_count,
            "missing_or_partial_modules": missing_or_partial_modules,
            "chart_runtime": chart_runtime_manifest,
            "data_snapshot_source": data_snapshot_source
        }
    })
}

fn render_module_html(
    module: Value,
    index: usize,
    mobile_order: &[String],
    data_snapshot: &Value,
) -> String {
    let title = module
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("未命名模块");
    let content = module
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("等待模型补齐内容。");
    let data_label = module
        .get("dataBinding")
        .or_else(|| module.get("data_binding"))
        .and_then(|binding| binding.get("label"))
        .and_then(Value::as_str)
        .unwrap_or("数据绑定待确认");
    let visualization = module
        .get("visualization")
        .and_then(|visualization| visualization.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("text-insight");
    let visualization_label = module
        .get("visualization")
        .and_then(|visualization| visualization.get("label"))
        .and_then(Value::as_str)
        .unwrap_or(visualization);
    let chart_runtime = module_chart_runtime(&module);
    let data_quality = module_data_quality(&module, data_snapshot);
    let render_fallback = module_chart_runtime_fallback(chart_runtime);
    let module_id = module.get("id").and_then(Value::as_str).unwrap_or("module");
    let layout_style = module_layout_style(&module, module_id, index, mobile_order);
    let chart_html = render_visualization_html(&module, visualization, data_snapshot);
    let chart_body =
        render_chart_body(&module, module_id, visualization, data_snapshot, chart_html);
    format!(
        concat!(
            "<section class=\"module\" data-chart=\"{}\" data-chart-runtime=\"{}\" data-data-quality=\"{}\" data-render-fallback=\"{}\" data-module-id=\"{}\" style=\"{}\">",
            "<div><span>{}</span><h2>{}</h2></div>",
            "<p>{}</p><small>{}</small>",
            "<div class=\"chart\">{}</div></section>"
        ),
        escape_html(visualization),
        escape_html(chart_runtime),
        escape_html(&data_quality),
        escape_html(render_fallback),
        escape_html(module_id),
        escape_html(&layout_style),
        escape_html(visualization_label),
        escape_html(title),
        escape_html(content),
        escape_html(data_label),
        chart_body,
    )
}

fn render_chart_body(
    module: &Value,
    module_id: &str,
    visualization: &str,
    data_snapshot: &Value,
    fallback_html: String,
) -> String {
    let Some(option) = echarts_option_for_module(module, visualization, data_snapshot) else {
        return fallback_html;
    };
    let option_json = escape_json_script(&option.to_string());
    format!(
        concat!(
            "<div class=\"echarts-hydration-target\" data-echarts-module-id=\"{}\" hidden></div>",
            "<div class=\"chart-fallback\">{}</div>",
            "<script type=\"application/json\" class=\"static-page-echarts-option\" data-module-id=\"{}\">{}</script>"
        ),
        escape_html(module_id),
        fallback_html,
        escape_html(module_id),
        option_json,
    )
}

fn build_chart_runtime_manifest(modules: &Value, data_snapshot: &Value) -> Value {
    let mut deterministic_count = 0;
    let mut echarts_requested_count = 0;
    let mut echarts_hydratable_count = 0;
    let mut fallback_count = 0;
    let mut data_quality_counts = BTreeMap::<String, usize>::new();
    let module_manifests = modules
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|module| {
            let module_id = module
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("module")
                .to_string();
            let visualization_type = module
                .get("visualization")
                .and_then(|visualization| visualization.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("text-insight")
                .to_string();
            let chart_runtime = module_chart_runtime(&module).to_string();
            let data_quality = module_data_quality(&module, data_snapshot);
            *data_quality_counts.entry(data_quality.clone()).or_insert(0) += 1;
            let echarts_option =
                echarts_option_for_module(&module, &visualization_type, data_snapshot);
            if chart_runtime == "echarts" {
                echarts_requested_count += 1;
                if echarts_option.is_some() {
                    echarts_hydratable_count += 1;
                }
                fallback_count += 1;
            } else {
                deterministic_count += 1;
            }
            let fallback = chart_runtime == "echarts";
            let fallback_runtime = module_chart_runtime_fallback(&chart_runtime);
            let final_renderer_runtime = if chart_runtime == "echarts" {
                "deterministic-svg-html+echarts-json-hydration"
            } else {
                "deterministic-svg-html"
            };
            json!({
                "moduleId": module_id,
                "visualizationType": visualization_type,
                "chartRuntime": chart_runtime,
                "dataQuality": data_quality,
                "finalRendererRuntime": final_renderer_runtime,
                "fallback": fallback,
                "fallbackRuntime": fallback_runtime,
                "echartsHydratable": echarts_option.is_some(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "defaultRuntime": "deterministic",
        "supportedRuntimes": ["deterministic", "echarts"],
        "finalRendererRuntime": "deterministic-svg-html+optional-echarts-json-hydration",
        "deterministicModules": deterministic_count,
        "echartsRequestedModules": echarts_requested_count,
        "echartsRenderedModules": 0,
        "echartsHydratableModules": echarts_hydratable_count,
        "fallbackModules": fallback_count,
        "dataQualityCounts": data_quality_counts,
        "modules": module_manifests,
    })
}

fn module_chart_runtime(module: &Value) -> &'static str {
    let runtime = module
        .get("visualization")
        .and_then(|visualization| {
            visualization
                .get("chartRuntime")
                .or_else(|| visualization.get("runtime"))
                .or_else(|| {
                    visualization
                        .get("chartOptions")
                        .and_then(|chart_options| chart_options.get("chartRuntime"))
                })
                .or_else(|| {
                    visualization
                        .get("chartOptions")
                        .and_then(|chart_options| chart_options.get("runtime"))
                })
        })
        .or_else(|| module.get("chartRuntime"))
        .or_else(|| {
            module
                .get("chartOptions")
                .and_then(|chart_options| chart_options.get("chartRuntime"))
        })
        .and_then(Value::as_str)
        .map(str::trim);
    match runtime {
        Some("echarts") => "echarts",
        _ => "deterministic",
    }
}

fn module_data_quality(module: &Value, data_snapshot: &Value) -> String {
    let module_id = module.get("id").and_then(Value::as_str);
    let data_quality = module_id
        .and_then(|id| data_snapshot_module_binding(data_snapshot, id))
        .and_then(|binding| {
            binding
                .get("dataQuality")
                .or_else(|| binding.get("data_quality"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if chart_points(module, data_snapshot).is_empty() {
                "missing"
            } else {
                "module_data"
            }
        });
    data_quality.to_string()
}

fn module_chart_runtime_fallback(chart_runtime: &str) -> &'static str {
    if chart_runtime == "echarts" {
        "deterministic-svg-html"
    } else {
        "none"
    }
}

fn render_echarts_hydration_script(modules: &Value, data_snapshot: &Value) -> String {
    let has_echarts_module = modules
        .as_array()
        .map(|items| {
            items.iter().any(|module| {
                let visualization = module
                    .get("visualization")
                    .and_then(|visualization| visualization.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("text-insight");
                echarts_option_for_module(module, visualization, data_snapshot).is_some()
            })
        })
        .unwrap_or(false);
    if !has_echarts_module {
        return String::new();
    }
    concat!(
        "<script>(function(){",
        "function hydrate(){if(!window.echarts)return;",
        "document.querySelectorAll('.static-page-echarts-option').forEach(function(node){",
        "var parent=node.parentElement;var target=parent&&parent.querySelector('.echarts-hydration-target');if(!target)return;",
        "try{var option=JSON.parse(node.textContent||'{}');var chart=window.echarts.init(target,null,{renderer:'canvas'});",
        "chart.setOption(option,true);target.hidden=false;target.setAttribute('aria-hidden','false');",
        "var fallback=parent.querySelector('.chart-fallback');if(fallback)fallback.hidden=true;",
        "window.addEventListener('resize',function(){chart.resize();});}catch(error){target.hidden=true;}",
        "});}",
        "window.__staticPageHydrateEcharts=hydrate;",
        "if(document.readyState==='loading'){document.addEventListener('DOMContentLoaded',hydrate);}else{hydrate();}",
        "})();</script>"
    )
    .to_string()
}

fn echarts_option_for_module(
    module: &Value,
    visualization: &str,
    data_snapshot: &Value,
) -> Option<Value> {
    if module_chart_runtime(module) != "echarts" {
        return None;
    }
    let points = chart_points(module, data_snapshot);
    let fallback = default_echarts_option(visualization, &points);
    let sanitized = module_chart_options(module)
        .map(sanitize_echarts_options)
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut option = fallback.unwrap_or_else(|| json!({"animation": false}));
    if echarts_option_has_data(&sanitized) {
        merge_object_values(&mut option, sanitized);
    }
    if echarts_option_has_data(&option) {
        Some(option)
    } else {
        None
    }
}

fn module_chart_options(module: &Value) -> Option<&Value> {
    module
        .get("visualization")
        .and_then(|visualization| {
            visualization
                .get("chartOptions")
                .or_else(|| visualization.get("chart_options"))
        })
        .or_else(|| module.get("chartOptions"))
        .or_else(|| module.get("chart_options"))
}

fn default_echarts_option(visualization: &str, points: &[ChartPoint]) -> Option<Value> {
    if points.is_empty() {
        return None;
    }
    let labels = points
        .iter()
        .map(|point| Value::String(point.label.clone()))
        .collect::<Vec<_>>();
    let values = points
        .iter()
        .map(|point| json!(point.value))
        .collect::<Vec<_>>();
    match visualization {
        "donut-chart" => Some(json!({
            "animation": false,
            "tooltip": { "trigger": "item" },
            "series": [{
                "type": "pie",
                "radius": ["46%", "72%"],
                "data": points.iter().map(|point| json!({
                    "name": point.label,
                    "value": point.value
                })).collect::<Vec<_>>()
            }]
        })),
        "line-chart" => Some(json!({
            "animation": false,
            "tooltip": { "trigger": "axis" },
            "grid": { "left": 28, "right": 16, "top": 22, "bottom": 28, "containLabel": true },
            "xAxis": { "type": "category", "data": labels },
            "yAxis": { "type": "value" },
            "series": [{ "type": "line", "data": values }]
        })),
        "bar-chart" | "risk-matrix" | "kpi-cards" | "table" | "timeline" => Some(json!({
            "animation": false,
            "tooltip": { "trigger": "axis" },
            "grid": { "left": 28, "right": 16, "top": 22, "bottom": 28, "containLabel": true },
            "xAxis": { "type": "category", "data": labels },
            "yAxis": { "type": "value" },
            "series": [{ "type": "bar", "data": values }]
        })),
        _ => None,
    }
}

fn sanitize_echarts_options(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Object(Map::new());
    };
    let mut output = Map::new();
    for (key, item) in object {
        if !is_allowed_echarts_top_level_key(key) {
            continue;
        }
        if key == "series" {
            let series = sanitize_echarts_series(item);
            if !series.is_empty() {
                output.insert(key.clone(), Value::Array(series));
            }
            continue;
        }
        if let Some(sanitized) = sanitize_chart_json_value(item) {
            output.insert(key.clone(), sanitized);
        }
    }
    Value::Object(output)
}

fn sanitize_echarts_series(value: &Value) -> Vec<Value> {
    let items = value
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![value.clone()]);
    items
        .into_iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let series_type = object.get("type").and_then(Value::as_str)?;
            if !is_allowed_echarts_series_type(series_type) {
                return None;
            }
            sanitize_chart_json_value(&Value::Object(object.clone()))
        })
        .collect()
}

fn sanitize_chart_json_value(value: &Value) -> Option<Value> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Some(value.clone()),
        Value::String(text) => is_safe_chart_string(text).then(|| Value::String(text.clone())),
        Value::Array(items) => Some(Value::Array(
            items
                .iter()
                .filter_map(sanitize_chart_json_value)
                .collect::<Vec<_>>(),
        )),
        Value::Object(object) => {
            let mut output = Map::new();
            for (key, item) in object {
                if is_dangerous_chart_key(key) {
                    continue;
                }
                if let Some(sanitized) = sanitize_chart_json_value(item) {
                    output.insert(key.clone(), sanitized);
                }
            }
            (!output.is_empty()).then(|| Value::Object(output))
        }
    }
}

fn is_allowed_echarts_top_level_key(key: &str) -> bool {
    matches!(
        key,
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
    )
}

fn is_allowed_echarts_series_type(series_type: &str) -> bool {
    matches!(
        series_type,
        "bar" | "line" | "pie" | "scatter" | "gauge" | "radar" | "heatmap" | "treemap"
    )
}

fn is_dangerous_chart_key(key: &str) -> bool {
    key == "__proto__"
        || key == "prototype"
        || key == "constructor"
        || key == "renderItem"
        || key.to_ascii_lowercase().starts_with("on")
}

fn is_safe_chart_string(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !(lower.contains('<')
        || lower.contains('>')
        || lower.contains("javascript:")
        || lower.contains("data:text/html")
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("@import")
        || lower.contains("expression("))
}

fn echarts_option_has_data(option: &Value) -> bool {
    option
        .get("dataset")
        .and_then(|dataset| dataset.get("source"))
        .and_then(Value::as_array)
        .map(|source| !source.is_empty())
        .unwrap_or(false)
        || option
            .get("series")
            .and_then(Value::as_array)
            .map(|series| {
                series.iter().any(|item| {
                    item.get("data")
                        .and_then(Value::as_array)
                        .map(|data| !data.is_empty())
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
}

fn merge_object_values(target: &mut Value, overlay: Value) {
    let (Some(target_object), Some(overlay_object)) = (target.as_object_mut(), overlay.as_object())
    else {
        return;
    };
    for (key, value) in overlay_object {
        target_object.insert(key.clone(), value.clone());
    }
}

fn module_layout_style(
    module: &Value,
    module_id: &str,
    index: usize,
    mobile_order: &[String],
) -> String {
    let layout = module.get("layout").unwrap_or(&Value::Null);
    let width = layout
        .get("w")
        .and_then(Value::as_i64)
        .unwrap_or(6)
        .clamp(1, 12);
    let x = layout
        .get("x")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .clamp(0, 12 - width);
    let height = layout
        .get("h")
        .and_then(Value::as_i64)
        .unwrap_or(3)
        .clamp(1, 8);
    let mobile_index = mobile_order
        .iter()
        .position(|id| id == module_id)
        .unwrap_or(index);
    format!(
        "grid-column:{} / span {};grid-row:span {};--mobile-order:{};min-height:{}px;",
        x + 1,
        width,
        height,
        mobile_index,
        112 + height * 24,
    )
}

fn render_visualization_html(module: &Value, visualization: &str, data_snapshot: &Value) -> String {
    match visualization {
        "kpi-cards" => render_kpi_cards(module, data_snapshot),
        "bar-chart" => render_bar_chart(module, data_snapshot),
        "line-chart" => render_line_chart(module, data_snapshot),
        "donut-chart" => render_donut_chart(module, data_snapshot),
        "table" => render_table(module, data_snapshot),
        "timeline" => render_timeline(module, data_snapshot),
        "risk-matrix" => render_risk_matrix(module, data_snapshot),
        "headline" => render_headline_visual(module),
        _ => render_text_insight_visual(module),
    }
}

fn render_kpi_cards(module: &Value, data_snapshot: &Value) -> String {
    let points = chart_points(module, data_snapshot);
    if points.is_empty() {
        return render_missing_data_notice(module);
    }
    let cards = points
        .iter()
        .take(4)
        .map(|point| {
            format!(
                "<span class=\"kpi-card\"><b>{}</b><i>{}</i></span>",
                escape_html(&format_number(point.value)),
                escape_html(&point.label)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<div class=\"kpi-grid\">{cards}</div>")
}

fn render_bar_chart(module: &Value, data_snapshot: &Value) -> String {
    let points = chart_points(module, data_snapshot);
    if points.is_empty() {
        return render_missing_data_notice(module);
    }
    let max_value = max_chart_value(&points);
    let bars = points
        .iter()
        .take(6)
        .enumerate()
        .map(|(index, point)| {
            let height = ((point.value / max_value) * 78.0).max(8.0);
            let x = 18 + index as i64 * 48;
            let y = 92.0 - height;
            format!(
                concat!(
                    "<g><rect x=\"{}\" y=\"{:.1}\" width=\"28\" height=\"{:.1}\" rx=\"7\"/>",
                    "<text x=\"{}\" y=\"108\" text-anchor=\"middle\">{}</text></g>"
                ),
                x,
                y,
                height,
                x + 14,
                escape_html(&short_label(&point.label, 4))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<svg class=\"chart-svg bar-chart\" viewBox=\"0 0 320 120\" role=\"img\" aria-label=\"柱状图\">{bars}</svg>"
    )
}

fn render_line_chart(module: &Value, data_snapshot: &Value) -> String {
    let points = chart_points(module, data_snapshot);
    if points.is_empty() {
        return render_missing_data_notice(module);
    }
    let max_value = max_chart_value(&points);
    let coords = points
        .iter()
        .take(7)
        .enumerate()
        .map(|(index, point)| {
            let x = 22.0 + index as f64 * 44.0;
            let y = 92.0 - ((point.value / max_value) * 72.0).max(4.0);
            (x, y, point)
        })
        .collect::<Vec<_>>();
    let path = coords
        .iter()
        .enumerate()
        .map(|(index, (x, y, _))| {
            if index == 0 {
                format!("M {:.1} {:.1}", x, y)
            } else {
                format!("L {:.1} {:.1}", x, y)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let dots = coords
        .iter()
        .map(|(x, y, point)| {
            format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\"><title>{}: {}</title></circle>",
                x,
                y,
                escape_html(&point.label),
                escape_html(&format_number(point.value))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        concat!(
            "<svg class=\"chart-svg line-chart\" viewBox=\"0 0 320 120\" role=\"img\" aria-label=\"趋势折线图\">",
            "<path class=\"line-area\" d=\"{} L 286 100 L 22 100 Z\"/>",
            "<path class=\"line-path\" d=\"{}\"/>{}</svg>"
        ),
        path, path, dots
    )
}

fn render_donut_chart(module: &Value, data_snapshot: &Value) -> String {
    let points = chart_points(module, data_snapshot);
    if points.is_empty() {
        return render_missing_data_notice(module);
    }
    let total = points.iter().map(|point| point.value.max(0.0)).sum::<f64>();
    let first = points
        .first()
        .map(|point| point.value.max(0.0))
        .unwrap_or(1.0);
    let ratio = if total > 0.0 { first / total } else { 0.58 };
    let dash = (ratio * 100.0).clamp(8.0, 92.0);
    let first_label = points
        .first()
        .map(|point| point.label.clone())
        .unwrap_or_else(|| "主要项".to_string());
    format!(
        concat!(
            "<svg class=\"chart-svg donut-chart\" viewBox=\"0 0 240 140\" role=\"img\" aria-label=\"占比环图\">",
            "<circle class=\"donut-base\" cx=\"70\" cy=\"70\" r=\"44\"/>",
            "<circle class=\"donut-value\" cx=\"70\" cy=\"70\" r=\"44\" stroke-dasharray=\"{:.1} 100\"/>",
            "<text class=\"donut-number\" x=\"70\" y=\"66\" text-anchor=\"middle\">{:.0}%</text>",
            "<text class=\"donut-label\" x=\"70\" y=\"84\" text-anchor=\"middle\">{}</text>",
            "<text class=\"donut-side\" x=\"138\" y=\"62\">{}</text>",
            "<text class=\"donut-side muted\" x=\"138\" y=\"84\">合计 {}</text></svg>"
        ),
        dash,
        dash,
        escape_html(&short_label(&first_label, 5)),
        escape_html(&first_label),
        escape_html(&format_number(total))
    )
}

fn render_table(module: &Value, data_snapshot: &Value) -> String {
    let points = chart_points(module, data_snapshot);
    if points.is_empty() {
        return render_missing_data_notice(module);
    }
    let rows = points
        .iter()
        .take(4)
        .map(|point| {
            format!(
                "<tr><td>{}</td><td>{}</td></tr>",
                escape_html(&point.label),
                escape_html(&format_number(point.value))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<table class=\"evidence-table\"><tbody>{rows}</tbody></table>")
}

fn render_timeline(module: &Value, data_snapshot: &Value) -> String {
    let points = chart_points(module, data_snapshot);
    if points.is_empty() {
        return render_missing_data_notice(module);
    }
    let items = points
        .iter()
        .take(4)
        .map(|point| {
            format!(
                "<li><b>{}</b><span>{}</span></li>",
                escape_html(&point.label),
                escape_html(&format!("{} 个相关信号", format_number(point.value)))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<ol class=\"timeline-chart\">{items}</ol>")
}

fn render_risk_matrix(module: &Value, data_snapshot: &Value) -> String {
    let points = chart_points(module, data_snapshot);
    if points.is_empty() {
        return render_missing_data_notice(module);
    }
    let chips = points
        .iter()
        .take(4)
        .enumerate()
        .map(|(index, point)| {
            let class_name = match index {
                0 => "risk-chip high",
                1 => "risk-chip medium",
                _ => "risk-chip low",
            };
            format!(
                "<span class=\"{}\"><b>{}</b><i>{}</i></span>",
                class_name,
                escape_html(&short_label(&point.label, 6)),
                escape_html(&format_number(point.value))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<div class=\"risk-matrix\">{chips}</div>")
}

fn render_headline_visual(module: &Value) -> String {
    let title = module
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("关键结论");
    format!(
        "<div class=\"headline-visual\"><b>{}</b><span>结论先行</span></div>",
        escape_html(&short_label(title, 12))
    )
}

fn render_text_insight_visual(module: &Value) -> String {
    let content = module
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("等待模型补齐内容。");
    format!(
        "<blockquote class=\"insight-quote\">{}</blockquote>",
        escape_html(&short_label(content, 42))
    )
}

fn render_missing_data_notice(module: &Value) -> String {
    let data_label = module
        .get("dataBinding")
        .or_else(|| module.get("data_binding"))
        .and_then(|binding| binding.get("label"))
        .and_then(Value::as_str)
        .unwrap_or("该模块数据");
    format!(
        "<div class=\"data-missing\"><b>数据待确认</b><span>{}</span></div>",
        escape_html(&short_label(data_label, 18))
    )
}

#[derive(Clone, Debug)]
struct ChartPoint {
    label: String,
    value: f64,
}

fn chart_points(module: &Value, data_snapshot: &Value) -> Vec<ChartPoint> {
    let module_id = module.get("id").and_then(Value::as_str);
    let candidates = [
        module
            .get("visualization")
            .and_then(|visualization| visualization.get("data")),
        module
            .get("visualization")
            .and_then(|visualization| visualization.get("values")),
        module
            .get("dataBinding")
            .or_else(|| module.get("data_binding")),
        module.get("data"),
        module_id.and_then(|id| data_snapshot_module_binding(data_snapshot, id)),
    ];

    for candidate in candidates.into_iter().flatten() {
        let points = chart_points_from_value(candidate);
        if !points.is_empty() {
            return points;
        }
        if let Some(binding) = candidate
            .get("binding")
            .or_else(|| candidate.get("dataBinding"))
        {
            let points = chart_points_from_value(binding);
            if !points.is_empty() {
                return points;
            }
        }
    }

    Vec::new()
}

fn data_snapshot_module_binding<'a>(
    data_snapshot: &'a Value,
    module_id: &str,
) -> Option<&'a Value> {
    data_snapshot
        .get("moduleBindings")
        .or_else(|| data_snapshot.get("module_bindings"))
        .and_then(Value::as_array)
        .and_then(|bindings| {
            bindings.iter().find(|binding| {
                binding
                    .get("moduleId")
                    .or_else(|| binding.get("module_id"))
                    .and_then(Value::as_str)
                    == Some(module_id)
            })
        })
}

fn chart_points_from_value(value: &Value) -> Vec<ChartPoint> {
    if let Some(array) = value.as_array() {
        return chart_points_from_array(array);
    }
    for key in [
        "values",
        "data",
        "rows",
        "sampleData",
        "sample_data",
        "items",
    ] {
        if let Some(array) = value.get(key).and_then(Value::as_array) {
            let points = chart_points_from_array(array);
            if !points.is_empty() {
                return points;
            }
        }
    }
    Vec::new()
}

fn chart_points_from_array(array: &[Value]) -> Vec<ChartPoint> {
    array
        .iter()
        .enumerate()
        .filter_map(|(index, item)| chart_point_from_value(index, item))
        .collect()
}

fn chart_point_from_value(index: usize, value: &Value) -> Option<ChartPoint> {
    if let Some(number) = value.as_f64() {
        return Some(ChartPoint {
            label: format!("项{}", index + 1),
            value: number,
        });
    }
    let object = value.as_object()?;
    let label = [
        "label", "name", "month", "date", "period", "category", "title", "x", "月份", "日期",
        "分类",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(ToOwned::to_owned)
    .unwrap_or_else(|| format!("项{}", index + 1));
    let value = [
        "value",
        "amount",
        "count",
        "score",
        "rate",
        "total",
        "y",
        "订单金额",
        "金额",
        "收入",
        "数量",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(chart_json_number))
    .or_else(|| object.values().find_map(chart_json_number))?;
    Some(ChartPoint { label, value })
}

fn chart_json_number(value: &Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    text.trim_end_matches('%')
        .replace(',', "")
        .replace('，', "")
        .parse::<f64>()
        .ok()
}

fn max_chart_value(points: &[ChartPoint]) -> f64 {
    points
        .iter()
        .map(|point| point.value.abs())
        .fold(1.0, f64::max)
}

fn format_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn short_label(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let count = trimmed.chars().count();
    if count <= max_chars {
        return trimmed.to_string();
    }
    let mut next = trimmed.chars().take(max_chars).collect::<String>();
    next.push('…');
    next
}

fn static_page_payload_modules(payload: &Value) -> Value {
    payload
        .get("modules")
        .and_then(Value::as_array)
        .cloned()
        .map(Value::Array)
        .unwrap_or_else(|| Value::Array(Vec::new()))
}

fn static_page_payload_string(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        payload
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn static_page_payload_string_array(payload: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_array))
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn static_page_payload_value(payload: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| payload.get(*key).cloned())
}

fn fallback_visual_spec(style: &str) -> Value {
    match style {
        "decision-brief" => json!({
            "palette": {
                "background": "#0f172a",
                "surface": "rgba(255,255,255,0.08)",
                "text": "#f8fafc",
                "mutedText": "#cbd5e1",
                "accent": "#93c5fd",
                "chart": "#38bdf8"
            },
            "surface": {
                "radius": 26
            }
        }),
        "data-command" => json!({
            "palette": {
                "background": "#064e3b",
                "surface": "rgba(255,255,255,0.09)",
                "text": "#ecfeff",
                "mutedText": "#cbd5e1",
                "accent": "#5eead4",
                "chart": "#22d3ee"
            },
            "surface": {
                "radius": 22
            }
        }),
        _ => json!({
            "palette": {
                "background": "#f7f8fb",
                "surface": "rgba(255,255,255,0.76)",
                "text": "#101827",
                "mutedText": "#475569",
                "accent": "#2563eb",
                "chart": "#0ea5e9"
            },
            "surface": {
                "radius": 26
            }
        }),
    }
}

fn fallback_render_spec() -> Value {
    json!({
        "renderer": STATIC_PAGE_RENDERER_ID,
        "layoutEngine": "css-grid-12",
        "componentModel": "dom-text-svg-chart",
        "chartRuntime": "deterministic-with-echarts-advanced",
        "chartRuntimePolicy": {
            "default": "deterministic",
            "advanced": "echarts",
            "finalRendererFallback": "deterministic-svg-html",
            "advancedHydration": "safe-echarts-json-option-island"
        },
    })
}

fn static_page_visual_style_attr(visual_spec: &Value) -> String {
    let background = nested_string(visual_spec, &["palette", "background"], "#f7f8fb");
    let surface = nested_string(
        visual_spec,
        &["palette", "surface"],
        "rgba(255,255,255,0.76)",
    );
    let text = nested_string(visual_spec, &["palette", "text"], "#101827");
    let muted = nested_string(visual_spec, &["palette", "mutedText"], "#475569");
    let accent = nested_string(visual_spec, &["palette", "accent"], "#2563eb");
    let chart = nested_string(visual_spec, &["palette", "chart"], "#0ea5e9");
    let radius = nested_number(visual_spec, &["surface", "radius"], 26.0);
    format!(
        "--static-page-bg:{background};--static-page-surface:{surface};--static-page-text:{text};--static-page-muted:{muted};--static-page-accent:{accent};--static-page-chart:{chart};--static-page-radius:{radius}px;"
    )
}

fn nested_string(value: &Value, path: &[&str], fallback: &str) -> String {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback.to_string();
        };
        current = next;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn nested_number(value: &Value, path: &[&str], fallback: f64) -> f64 {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_f64().unwrap_or(fallback)
}

fn style_label(style: &str) -> &'static str {
    match style {
        "decision-brief" => "高层决策简报",
        "data-command" => "数据运营看板",
        _ => "客户交付报告",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_json_script(value: &str) -> String {
    value
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

const STATIC_PAGE_RENDER_CSS: &str = r#"
:root{color-scheme:light;font-family:Aptos,ui-sans-serif,system-ui,sans-serif;background:#f7f8fb;color:#101827}
*{box-sizing:border-box}body{margin:0;padding:28px;background:var(--static-page-bg,linear-gradient(135deg,#f7fbff,#fff7ed));color:var(--static-page-text,#101827)}
body.style-decision-brief{background:linear-gradient(135deg,#0f172a,#1e293b);color:#f8fafc}
body.style-data-command{background:linear-gradient(135deg,#064e3b,#0f172a);color:#ecfeff}
main{max-width:1160px;margin:0 auto;display:grid;gap:18px}.module-grid{display:grid;grid-template-columns:repeat(12,minmax(0,1fr));grid-auto-rows:minmax(68px,auto);grid-auto-flow:dense;gap:18px}.cover,.module{border-radius:var(--static-page-radius,26px);padding:24px;background:var(--static-page-surface,rgba(255,255,255,.72));box-shadow:0 20px 70px rgba(15,23,42,.12)}
.style-decision-brief .cover,.style-decision-brief .module,.style-data-command .cover,.style-data-command .module{background:rgba(255,255,255,.08);box-shadow:0 20px 70px rgba(0,0,0,.22)}
.cover span,.module span,small{font-size:12px;font-weight:800;letter-spacing:.08em;text-transform:uppercase;color:var(--static-page-accent,#2563eb)}
.style-decision-brief .cover span,.style-decision-brief .module span,.style-data-command .cover span,.style-data-command .module span{color:#93c5fd}
h1{font-size:clamp(32px,6vw,64px);line-height:.95;margin:10px 0 14px}h2{font-size:24px;margin:4px 0 0}p{line-height:1.7;margin:0;color:var(--static-page-muted,#475569)}
.style-decision-brief p,.style-data-command p,.style-decision-brief small,.style-data-command small{color:#cbd5e1}
.module{display:grid;gap:14px;align-content:start;overflow:hidden}.chart{min-height:92px;border-radius:18px;display:grid;place-items:center;background:linear-gradient(135deg,rgba(37,99,235,.12),rgba(14,165,233,.08));font-weight:900;color:var(--static-page-chart,#1d4ed8);padding:10px}
.style-decision-brief .chart,.style-data-command .chart{background:rgba(255,255,255,.1);color:#bfdbfe}
.echarts-hydration-target{width:100%;min-height:190px}.chart-fallback{width:100%;display:grid;place-items:center}.static-page-echarts-option{display:none!important}
.chart-svg{width:100%;height:100%;min-height:112px;overflow:visible}.chart-svg rect,.chart-svg .donut-value{fill:var(--static-page-chart,#0ea5e9)}.chart-svg text{font-size:10px;fill:var(--static-page-muted,#475569);font-weight:800}.style-decision-brief .chart-svg text,.style-data-command .chart-svg text{fill:#cbd5e1}.line-path{fill:none;stroke:var(--static-page-chart,#0ea5e9);stroke-width:5;stroke-linecap:round;stroke-linejoin:round}.line-area{fill:var(--static-page-chart,#0ea5e9);opacity:.13}.line-chart circle{fill:var(--static-page-surface,#fff);stroke:var(--static-page-chart,#0ea5e9);stroke-width:3}.donut-base{fill:none;stroke:rgba(100,116,139,.22);stroke-width:18}.donut-value{fill:none;stroke:var(--static-page-chart,#0ea5e9);stroke-width:18;transform:rotate(-90deg);transform-origin:70px 70px;stroke-linecap:round}.donut-number{font-size:24px!important;fill:var(--static-page-text,#101827)!important}.donut-label,.donut-side{font-size:11px!important}.donut-side.muted{opacity:.68}.kpi-grid{width:100%;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}.kpi-card{display:grid;gap:2px;padding:12px;border-radius:16px;background:rgba(255,255,255,.42)}.style-decision-brief .kpi-card,.style-data-command .kpi-card{background:rgba(255,255,255,.1)}.kpi-card b{font-size:24px;color:var(--static-page-chart,#0ea5e9)}.kpi-card i{font-style:normal;font-size:11px;color:var(--static-page-muted,#475569)}.data-missing{display:grid;gap:6px;text-align:center;color:var(--static-page-muted,#475569)}.data-missing b{font-size:18px;color:var(--static-page-text,#101827)}.data-missing span{font-size:12px}.evidence-table{width:100%;border-collapse:collapse;font-size:12px}.evidence-table td{padding:9px 10px;border-bottom:1px solid rgba(100,116,139,.2)}.timeline-chart{width:100%;margin:0;padding:0;display:grid;gap:9px;list-style:none}.timeline-chart li{display:flex;gap:10px;align-items:center}.timeline-chart b{min-width:56px;color:var(--static-page-chart,#0ea5e9)}.timeline-chart span{color:var(--static-page-muted,#475569);font-size:12px}.risk-matrix{width:100%;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}.risk-chip{display:grid;gap:4px;padding:11px;border-radius:16px;background:rgba(100,116,139,.16)}.risk-chip.high{background:rgba(239,68,68,.18)}.risk-chip.medium{background:rgba(245,158,11,.18)}.risk-chip.low{background:rgba(14,165,233,.16)}.risk-chip i{font-style:normal;font-size:11px;color:var(--static-page-muted,#475569)}.headline-visual{display:grid;gap:6px;text-align:center}.headline-visual b{font-size:28px;color:var(--static-page-text,#101827)}.headline-visual span{font-size:12px;color:var(--static-page-accent,#2563eb)}.insight-quote{margin:0;padding:0 0 0 14px;border-left:4px solid var(--static-page-chart,#0ea5e9);font-size:14px;line-height:1.6;color:var(--static-page-muted,#475569)}
@media(max-width:720px){body{padding:14px}.module-grid{display:flex;flex-direction:column}.module{grid-column:1/-1!important;grid-row:auto!important;order:var(--mobile-order);min-height:auto!important}.cover,.module{padding:18px;border-radius:20px}h1{font-size:36px}.kpi-grid,.risk-matrix{grid-template-columns:1fr}}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_static_page_outputs_html_and_manifest() {
        let result = render_static_page(&StaticPageRenderRequest {
            draft_id: "draft-1".to_string(),
            assistant_run_id: "run-1".to_string(),
            title: "经营分析静态页".to_string(),
            draft_payload: json!({
                "styleDirection": "decision-brief",
                "visualSpec": {
                    "palette": {
                        "background": "#111827",
                        "surface": "rgba(255,255,255,0.1)",
                        "text": "#f8fafc",
                        "mutedText": "#d1d5db",
                        "accent": "#facc15",
                        "chart": "#facc15"
                    },
                    "surface": {
                        "radius": 30
                    }
                },
                "renderSpec": {
                    "renderer": "static-page-renderer-v1",
                    "componentModel": "dom-text-svg-chart"
                },
                "previewContract": {
                    "status": "confirmed",
                    "assetKey": "previews/static-page-1.png"
                },
                "modelSummary": "核心增长来自高价值客户。",
                "mobileOrder": ["trend", "hero"],
                "modules": [{
                    "id": "hero",
                    "title": "核心判断",
                    "content": "增长放缓但结构改善。",
                    "dataBinding": { "label": "订单数据摘要" },
                    "visualization": { "type": "headline", "label": "关键结论" },
                    "layout": { "x": 0, "y": 0, "w": 5, "h": 3 }
                }, {
                    "id": "trend",
                    "title": "趋势变化",
                    "content": "订单转化率连续三周回升。",
                    "dataBinding": {
                        "label": "订单趋势",
                        "values": [
                            { "label": "一月", "value": 42 },
                            { "label": "二月", "value": 58 },
                            { "label": "三月", "value": 76 }
                        ]
                    },
                    "visualization": { "type": "bar-chart", "label": "分类对比柱状图" },
                    "layout": { "x": 5, "y": 0, "w": 7, "h": 3 }
                }]
            }),
            selected_scope: json!({"mode": "selected_datasets"}),
            visibility_snapshot: json!({"policy": "assistant_run_scope_snapshot"}),
            preview_asset_key: Some("previews/static-page-1.png".to_string()),
            image_job_id: Some("job-1".to_string()),
        });

        assert!(result.html.contains("核心判断"));
        assert!(result.html.contains("previews/static-page-1.png"));
        assert_eq!(result.asset_manifest["renderer"], STATIC_PAGE_RENDERER_ID);
        assert_eq!(
            result.asset_manifest["design_contract"]["final_role"],
            "html_css_svg_renderer"
        );
        assert_eq!(
            result.asset_manifest["visual_spec"]["palette"]["accent"],
            "#facc15"
        );
        assert!(result.html.contains("class=\"module-grid\""));
        assert!(result.html.contains("grid-column:6 / span 7"));
        assert!(result.html.contains("--mobile-order:0"));
        assert!(result.html.contains("bar-chart"));
        assert!(result.html.contains("<svg class=\"chart-svg bar-chart\""));
        assert!(result.html.contains("一月"));
        assert!(result.html.contains("--static-page-accent:#facc15"));
        assert_eq!(result.asset_manifest["module_count"], 2);
        assert_eq!(
            result.asset_manifest["chart_runtime"]["deterministicModules"],
            2
        );
        assert_eq!(result.asset_manifest["chart_runtime"]["fallbackModules"], 0);
        assert_eq!(
            result.asset_manifest["export_package"]["kind"],
            "static-page-export-package"
        );
        assert_eq!(
            result.asset_manifest["export_package"]["files"][0]["path"],
            "index.html"
        );
        assert_eq!(
            result.asset_manifest["export_package"]["debug"]["module_count"],
            2
        );
        assert_eq!(
            result.asset_manifest["export_package"]["runtime_requirements"][0]["license"],
            "Apache-2.0"
        );
        assert_eq!(
            result.asset_manifest["export_package"]["runtime_requirements"][0]["required"],
            false
        );
    }

    #[test]
    fn render_static_page_records_echarts_fallback_manifest() {
        let result = render_static_page(&StaticPageRenderRequest {
            draft_id: "draft-echarts".to_string(),
            assistant_run_id: "run-echarts".to_string(),
            title: "高级图表测试".to_string(),
            draft_payload: json!({
                "modules": [{
                    "id": "trend",
                    "title": "订单趋势",
                    "content": "ECharts 预览确认后，最终渲染仍保留静态回退。",
                    "visualization": {
                        "type": "bar-chart",
                        "chartRuntime": "echarts",
                        "chartOptions": {
                            "title": { "text": "订单趋势" },
                            "series": [{
                                "type": "bar",
                                "data": [1200, 1380]
                            }]
                        }
                    }
                }],
                "dataSnapshot": {
                    "module_bindings": [{
                        "moduleId": "trend",
                        "sampleData": [
                            { "label": "1月", "value": 1200, "kind": "module_data" },
                            { "label": "2月", "value": 1380, "kind": "module_data" }
                        ],
                        "dataQuality": "module_data"
                    }]
                }
            }),
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            preview_asset_key: None,
            image_job_id: None,
        });

        assert!(result.html.contains("data-chart-runtime=\"echarts\""));
        assert!(result
            .html
            .contains("data-render-fallback=\"deterministic-svg-html\""));
        assert!(result.html.contains("static-page-echarts-option"));
        assert!(result.html.contains("echarts-hydration-target"));
        assert!(result.html.contains("window.echarts"));
        assert!(result.html.contains("__staticPageHydrateEcharts"));
        assert!(result.html.contains("\"title\":{\"text\":\"订单趋势\"}"));
        assert_eq!(
            result.asset_manifest["chart_runtime"]["echartsRequestedModules"],
            1
        );
        assert_eq!(
            result.asset_manifest["chart_runtime"]["echartsHydratableModules"],
            1
        );
        assert_eq!(result.asset_manifest["chart_runtime"]["fallbackModules"], 1);
        assert_eq!(
            result.asset_manifest["chart_runtime"]["modules"][0]["dataQuality"],
            "module_data"
        );
        assert_eq!(
            result.asset_manifest["chart_runtime"]["modules"][0]["echartsHydratable"],
            true
        );
        assert_eq!(
            result.asset_manifest["chart_runtime"]["modules"][0]["finalRendererRuntime"],
            "deterministic-svg-html+echarts-json-hydration"
        );
        assert!(result.html.contains("1月"));
        assert_eq!(
            result.asset_manifest["data_snapshot"]["module_bindings"][0]["sampleData"][0]["value"],
            json!(1200)
        );
    }

    #[test]
    fn render_static_page_sanitizes_echarts_json_islands() {
        let result = render_static_page(&StaticPageRenderRequest {
            draft_id: "draft-echarts-unsafe".to_string(),
            assistant_run_id: "run-echarts-unsafe".to_string(),
            title: "高级图表安全测试".to_string(),
            draft_payload: json!({
                "modules": [{
                    "id": "trend",
                    "title": "订单趋势",
                    "content": "安全保留 ECharts JSON。",
                    "visualization": {
                        "type": "bar-chart",
                        "chartRuntime": "echarts",
                        "chartOptions": {
                            "title": { "text": "<img src=x onerror=alert(1)>" },
                            "tooltip": { "formatter": "javascript:alert(1)" },
                            "series": [{
                                "type": "bar",
                                "data": [1200, 1380],
                                "itemStyle": { "color": "https://example.com/evil.js" },
                                "renderItem": "alert(1)"
                            }, {
                                "type": "custom",
                                "data": [1]
                            }]
                        }
                    }
                }]
            }),
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            preview_asset_key: None,
            image_job_id: None,
        });

        assert!(result.html.contains("static-page-echarts-option"));
        assert!(result.html.contains("\"series\":[{"));
        assert!(result.html.contains("\"type\":\"bar\""));
        assert!(result.html.contains("\"data\":[1200,1380]"));
        assert!(!result.html.contains("javascript:"));
        assert!(!result.html.contains("https://example.com"));
        assert!(!result.html.contains("<img"));
        assert!(!result.html.contains("itemStyle"));
        assert!(!result.html.contains("renderItem"));
        assert!(!result.html.contains("\"custom\""));
    }

    #[test]
    fn render_static_page_supports_layout_and_chart_variants() {
        let result = render_static_page(&StaticPageRenderRequest {
            draft_id: "draft-2".to_string(),
            assistant_run_id: "run-2".to_string(),
            title: "图表测试".to_string(),
            draft_payload: json!({
                "styleDirection": "data-command",
                "dataSnapshot": {
                    "moduleBindings": [{
                        "moduleId": "risk",
                        "binding": {
                            "values": [
                                { "label": "交付延期", "value": 88 },
                                { "label": "成本波动", "value": 56 },
                                { "label": "客服积压", "value": 34 }
                            ]
                        }
                    }]
                },
                "modules": [{
                    "id": "kpi",
                    "title": "关键指标",
                    "content": "展示四个指标。",
                    "visualization": {
                        "type": "kpi-cards",
                        "data": [12, 24, 36, 48]
                    },
                    "layout": { "x": 0, "w": 4, "h": 2 }
                }, {
                    "id": "share",
                    "title": "结构占比",
                    "content": "展示主要占比。",
                    "visualization": {
                        "type": "donut-chart",
                        "data": [
                            { "name": "高价值客户", "amount": 64 },
                            { "name": "普通客户", "amount": 36 }
                        ]
                    },
                    "layout": { "x": 4, "w": 4, "h": 2 }
                }, {
                    "id": "risk",
                    "title": "风险矩阵",
                    "content": "展示优先风险。",
                    "visualization": { "type": "risk-matrix" },
                    "layout": { "x": 8, "w": 4, "h": 2 }
                }]
            }),
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            preview_asset_key: None,
            image_job_id: None,
        });

        assert!(result.html.contains("kpi-grid"));
        assert!(result.html.contains("donut-chart"));
        assert!(result.html.contains("risk-matrix"));
        assert!(result.html.contains("交付延期"));
    }

    #[test]
    fn render_static_page_escapes_module_text() {
        let result = render_static_page(&StaticPageRenderRequest {
            draft_id: "draft-1".to_string(),
            assistant_run_id: "run-1".to_string(),
            title: "<script>alert(1)</script>".to_string(),
            draft_payload: json!({
                "modules": [{
                    "title": "<b>危险</b>",
                    "content": "\"quoted\" & raw",
                }]
            }),
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            preview_asset_key: None,
            image_job_id: None,
        });

        assert!(result
            .html
            .contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(result.html.contains("&lt;b&gt;危险&lt;/b&gt;"));
        assert!(result.html.contains("&quot;quoted&quot; &amp; raw"));
    }

    #[test]
    fn render_static_page_labels_missing_chart_data_without_fake_values() {
        let result = render_static_page(&StaticPageRenderRequest {
            draft_id: "draft-missing".to_string(),
            assistant_run_id: "run-missing".to_string(),
            title: "缺失数据测试".to_string(),
            draft_payload: json!({
                "modules": [{
                    "id": "trend",
                    "title": "趋势变化",
                    "content": "等待检索到趋势数据。",
                    "dataBinding": { "label": "订单趋势" },
                    "visualization": { "type": "line-chart" }
                }]
            }),
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            preview_asset_key: None,
            image_job_id: None,
        });

        assert!(result.html.contains("data-missing"));
        assert!(result.html.contains("数据待确认"));
        assert!(!result.html.contains(">当前<"));
    }

    #[test]
    fn render_static_page_uses_snapshot_sample_data_values() {
        let result = render_static_page(&StaticPageRenderRequest {
            draft_id: "draft-real-values".to_string(),
            assistant_run_id: "run-real-values".to_string(),
            title: "真实数据测试".to_string(),
            draft_payload: json!({
                "modules": [{
                    "id": "trend",
                    "title": "订单趋势",
                    "content": "使用检索证据里的显式数值。",
                    "dataBinding": {
                        "label": "订单金额",
                        "fieldPath": "orders.amount"
                    },
                    "visualization": { "type": "line-chart" }
                }],
                "dataSnapshot": {
                    "module_bindings": [{
                        "moduleId": "trend",
                        "sampleData": [
                            { "label": "1月", "value": 1200, "kind": "evidence_value" },
                            { "label": "2月", "value": 1380, "kind": "evidence_value" }
                        ],
                        "dataQuality": "evidence_value"
                    }]
                }
            }),
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            preview_asset_key: None,
            image_job_id: None,
        });

        assert!(result.html.contains("line-chart"));
        assert!(result.html.contains("1月: 1200"));
        assert!(result.html.contains("2月: 1380"));
        assert!(!result.html.contains("数据待确认"));
    }

    #[test]
    fn render_static_page_accepts_module_month_amount_data() {
        let result = render_static_page(&StaticPageRenderRequest {
            draft_id: "draft-module-values".to_string(),
            assistant_run_id: "run-module-values".to_string(),
            title: "模块数据测试".to_string(),
            draft_payload: json!({
                "modules": [{
                    "id": "trend",
                    "title": "订单趋势",
                    "content": "使用模块内用户确认的数据。",
                    "visualization": {
                        "type": "line-chart",
                        "data": [
                            { "month": "1月", "amount": "1200" },
                            { "month": "2月", "amount": "1380" }
                        ]
                    }
                }]
            }),
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            preview_asset_key: None,
            image_job_id: None,
        });

        assert!(result.html.contains("line-chart"));
        assert!(result.html.contains("1月: 1200"));
        assert!(result.html.contains("2月: 1380"));
        assert!(!result.html.contains("数据待确认"));
    }
}
