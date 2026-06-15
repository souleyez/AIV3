use serde_json::{json, Value};

use crate::static_page_dynamic_contract_support::build_static_page_report_time_range_contract;

pub(crate) fn build_static_page_visual_spec(style_direction: &str) -> Value {
    match style_direction {
        "decision-brief" => json!({
            "version": 1,
            "styleDirection": "decision-brief",
            "palette": {
                "background": "#0f172a",
                "surface": "rgba(255,255,255,0.08)",
                "text": "#f8fafc",
                "mutedText": "#cbd5e1",
                "accent": "#93c5fd",
                "chart": "#38bdf8"
            },
            "typography": {
                "headingFamily": "Aptos Display, ui-sans-serif, system-ui",
                "bodyFamily": "Aptos, ui-sans-serif, system-ui",
                "density": "compact"
            },
            "surface": {
                "radius": 26,
                "shadow": "deep",
                "decoration": "subtle-gradient"
            }
        }),
        "data-command" => json!({
            "version": 1,
            "styleDirection": "data-command",
            "palette": {
                "background": "#064e3b",
                "surface": "rgba(255,255,255,0.09)",
                "text": "#ecfeff",
                "mutedText": "#cbd5e1",
                "accent": "#5eead4",
                "chart": "#22d3ee"
            },
            "typography": {
                "headingFamily": "Aptos Display, ui-sans-serif, system-ui",
                "bodyFamily": "Aptos, ui-sans-serif, system-ui",
                "density": "dense"
            },
            "surface": {
                "radius": 22,
                "shadow": "glow",
                "decoration": "command-gradient"
            }
        }),
        _ => json!({
            "version": 1,
            "styleDirection": "client-delivery",
            "palette": {
                "background": "#f7f8fb",
                "surface": "rgba(255,255,255,0.76)",
                "text": "#101827",
                "mutedText": "#475569",
                "accent": "#2563eb",
                "chart": "#0ea5e9"
            },
            "typography": {
                "headingFamily": "Aptos Display, ui-sans-serif, system-ui",
                "bodyFamily": "Aptos, ui-sans-serif, system-ui",
                "density": "balanced"
            },
            "surface": {
                "radius": 26,
                "shadow": "soft",
                "decoration": "warm-gradient"
            }
        }),
    }
}

pub(crate) fn build_static_page_render_spec() -> Value {
    json!({
        "renderer": "static-page-renderer-v1",
        "layoutEngine": "css-grid-12",
        "desktopGrid": {
            "columns": 12,
            "rowHeight": 96
        },
        "mobileLayout": "single-column-sortable",
        "componentModel": "dom-text-svg-chart",
        "chartRuntime": "deterministic-with-echarts-advanced",
        "chartRuntimePolicy": {
            "default": "deterministic",
            "advanced": "echarts",
            "allowedRuntimes": ["deterministic", "echarts"],
            "advancedOptions": "plain-json-echarts-option-only",
            "finalRendererFallback": "ECharts modules must still have dataSnapshot sampleData so the final renderer can fall back to deterministic DOM/SVG output.",
            "advancedHydration": "Final HTML preserves safe ECharts JSON option islands; if an approved ECharts bundle is present, the page can hydrate charts without losing deterministic fallback."
        },
        "dynamicData": {
            "dataFile": "data.json",
            "sourceSnapshotFile": "data-snapshot.json",
            "clientRefresh": {
                "enabled": true,
                "intervalSeconds": 60,
                "changeDetectionFields": ["snapshotVersion", "updatedAt", "snapshot_version", "updated_at"]
            },
            "defaultControls": ["time_range", "primary_partition", "manual_refresh", "auto_refresh"],
            "reportTimeRange": build_static_page_report_time_range_contract(),
            "updateContract": "When datasets or source documents change, regenerate or replace data.json and let the final HTML re-render from the latest snapshot."
        },
        "editableContent": ["title", "content", "dataBinding", "visualization", "chartRuntime", "chartOptions", "layout"],
        "generationGuardrails": [
            "可视化必须服从模块网格布局和移动端顺序",
            "正文、指标、图表在最终静态页中必须是真 DOM 或 SVG，不允许只烘焙进图片",
            "复杂背景、纹理、装饰可以作为图片资产，核心数据表达必须可重新渲染",
            "ECharts 只允许纯 JSON 配置，不允许函数、HTML、远程 URL 或事件处理器字段",
            "避免 3D 透视、真实摄影 UI、不可复刻字体效果和过度复杂玻璃反射"
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_spec_uses_client_delivery_for_unknown_style() {
        let spec = build_static_page_visual_spec("unknown");

        assert_eq!(spec["version"], json!(1));
        assert_eq!(spec["styleDirection"], json!("client-delivery"));
        assert_eq!(spec["palette"]["background"], json!("#f7f8fb"));
        assert_eq!(spec["typography"]["density"], json!("balanced"));
        assert_eq!(spec["surface"]["decoration"], json!("warm-gradient"));
    }

    #[test]
    fn visual_spec_preserves_decision_brief_contract() {
        let spec = build_static_page_visual_spec("decision-brief");

        assert_eq!(spec["styleDirection"], json!("decision-brief"));
        assert_eq!(spec["palette"]["background"], json!("#0f172a"));
        assert_eq!(spec["typography"]["density"], json!("compact"));
        assert_eq!(spec["surface"]["shadow"], json!("deep"));
    }

    #[test]
    fn visual_spec_preserves_data_command_contract() {
        let spec = build_static_page_visual_spec("data-command");

        assert_eq!(spec["styleDirection"], json!("data-command"));
        assert_eq!(spec["palette"]["background"], json!("#064e3b"));
        assert_eq!(spec["typography"]["density"], json!("dense"));
        assert_eq!(spec["surface"]["shadow"], json!("glow"));
    }

    #[test]
    fn render_spec_contains_dynamic_data_and_chart_runtime_contracts() {
        let spec = build_static_page_render_spec();

        assert_eq!(spec["renderer"], json!("static-page-renderer-v1"));
        assert_eq!(spec["layoutEngine"], json!("css-grid-12"));
        assert_eq!(
            spec["chartRuntimePolicy"]["allowedRuntimes"],
            json!(["deterministic", "echarts"])
        );
        assert_eq!(spec["dynamicData"]["dataFile"], json!("data.json"));
        assert_eq!(
            spec["dynamicData"]["defaultControls"],
            json!([
                "time_range",
                "primary_partition",
                "manual_refresh",
                "auto_refresh"
            ])
        );
        assert_eq!(
            spec["dynamicData"]["reportTimeRange"]["default_preset"],
            json!("latest_available_month")
        );
        assert!(spec["generationGuardrails"]
            .as_array()
            .is_some_and(|items| items.len() == 5));
    }
}
