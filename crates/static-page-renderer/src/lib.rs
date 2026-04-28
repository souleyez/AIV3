use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    let preview = request.preview_asset_key.as_deref().unwrap_or("no-preview");
    let module_html = modules
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(render_module_html)
        .collect::<Vec<_>>()
        .join("\n");
    let html = format!(
        concat!(
            "<!doctype html><html><head><meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
            "<title>{}</title><style>{}</style></head>",
            "<body class=\"static-page style-{}\" data-preview=\"{}\">",
            "<main><header class=\"cover\"><span>{}</span><h1>{}</h1><p>{}</p></header>{}</main>",
            "</body></html>"
        ),
        escape_html(&request.title),
        STATIC_PAGE_RENDER_CSS,
        escape_html(&style),
        escape_html(preview),
        escape_html(style_label(&style)),
        escape_html(&request.title),
        escape_html(
            &static_page_payload_string(&request.draft_payload, &["modelSummary", "model_summary"])
                .unwrap_or_else(|| "按确认效果图和模块规划生成静态页。".to_string()),
        ),
        module_html,
    );
    let asset_manifest = json!({
        "draft_id": request.draft_id,
        "assistant_run_id": request.assistant_run_id,
        "style_direction": style,
        "preview_asset_key": request.preview_asset_key,
        "image_job_id": request.image_job_id,
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

fn render_module_html(module: Value) -> String {
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
    format!(
        concat!(
            "<section class=\"module\" data-chart=\"{}\">",
            "<div><span>{}</span><h2>{}</h2></div>",
            "<p>{}</p><small>{}</small>",
            "<div class=\"chart\">{}</div></section>"
        ),
        escape_html(visualization),
        escape_html(visualization_label),
        escape_html(title),
        escape_html(content),
        escape_html(data_label),
        escape_html(visualization_label),
    )
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

const STATIC_PAGE_RENDER_CSS: &str = r#"
:root{color-scheme:light;font-family:Inter,ui-sans-serif,system-ui,sans-serif;background:#f7f8fb;color:#101827}
*{box-sizing:border-box}body{margin:0;padding:28px;background:linear-gradient(135deg,#f7fbff,#fff7ed)}
body.style-decision-brief{background:linear-gradient(135deg,#0f172a,#1e293b);color:#f8fafc}
body.style-data-command{background:linear-gradient(135deg,#064e3b,#0f172a);color:#ecfeff}
main{max-width:1160px;margin:0 auto;display:grid;gap:18px}.cover,.module{border-radius:26px;padding:24px;background:rgba(255,255,255,.72);box-shadow:0 20px 70px rgba(15,23,42,.12)}
.style-decision-brief .cover,.style-decision-brief .module,.style-data-command .cover,.style-data-command .module{background:rgba(255,255,255,.08);box-shadow:0 20px 70px rgba(0,0,0,.22)}
.cover span,.module span,small{font-size:12px;font-weight:800;letter-spacing:.08em;text-transform:uppercase;color:#2563eb}
.style-decision-brief .cover span,.style-decision-brief .module span,.style-data-command .cover span,.style-data-command .module span{color:#93c5fd}
h1{font-size:clamp(32px,6vw,64px);line-height:.95;margin:10px 0 14px}h2{font-size:24px;margin:4px 0 0}p{line-height:1.7;margin:0;color:#475569}
.style-decision-brief p,.style-data-command p,.style-decision-brief small,.style-data-command small{color:#cbd5e1}
.module{display:grid;gap:14px}.chart{min-height:92px;border-radius:18px;display:grid;place-items:center;background:linear-gradient(135deg,rgba(37,99,235,.12),rgba(14,165,233,.08));font-weight:900;color:#1d4ed8}
.style-decision-brief .chart,.style-data-command .chart{background:rgba(255,255,255,.1);color:#bfdbfe}
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
                "modelSummary": "核心增长来自高价值客户。",
                "modules": [{
                    "id": "hero",
                    "title": "核心判断",
                    "content": "增长放缓但结构改善。",
                    "dataBinding": { "label": "订单数据摘要" },
                    "visualization": { "type": "headline", "label": "关键结论" }
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
        assert_eq!(result.asset_manifest["module_count"], 1);
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
}
