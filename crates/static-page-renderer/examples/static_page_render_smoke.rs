use serde_json::json;
use static_page_renderer::{render_static_page, StaticPageRenderRequest, STATIC_PAGE_RENDERER_ID};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/static-page-render-smoke/artifact"));
    fs::create_dir_all(&output_dir)?;

    let request = StaticPageRenderRequest {
        draft_id: "smoke-static-page-draft".to_string(),
        assistant_run_id: "smoke-assistant-run".to_string(),
        title: "新世界 IOA 问答运营静态页".to_string(),
        draft_payload: json!({
            "styleDirection": "client-delivery",
            "modelSummary": "基于已确认数据生成可交付静态页，最终 HTML 保留 DOM/SVG 回退与安全 ECharts JSON 岛。",
            "visualSpec": {
                "palette": {
                    "background": "#f8fafc",
                    "surface": "#ffffff",
                    "text": "#0f172a",
                    "muted": "#475569",
                    "accent": "#2563eb",
                    "chart": "#0ea5e9"
                }
            },
            "renderSpec": {
                "componentModel": "dom-text-svg-chart",
                "responsive": true
            },
            "previewContract": {
                "status": "confirmed",
                "imageJobId": "smoke-image-job",
                "assetKey": "previews/smoke-static-page.png",
                "draftFingerprint": "smoke-fingerprint",
                "confirmedAt": "2026-05-17T00:00:00Z"
            },
            "mobileOrder": ["overview", "answer-trend", "topic-share"],
            "modules": [{
                "id": "overview",
                "title": "问答服务总览",
                "content": "展示第三方资料库接入后的问答覆盖、命中和待补料状态。",
                "dataBinding": {
                    "label": "IOA 问答样本统计",
                    "sourceId": "dataset-ioa-qa",
                    "fieldPath": "qa.summary"
                },
                "visualization": {
                    "type": "kpi-cards",
                    "data": [
                        { "label": "知识段落", "value": "128" },
                        { "label": "权限命中", "value": "96%" },
                        { "label": "可回答问题", "value": "42" },
                        { "label": "需补料", "value": "3" }
                    ]
                },
                "layout": { "x": 0, "y": 0, "w": 4, "h": 3 }
            }, {
                "id": "answer-trend",
                "title": "最近问答命中趋势",
                "content": "按最近三轮测试样本展示模型基于 DataMax 资料回答的命中变化。",
                "dataBinding": {
                    "label": "问答命中率",
                    "sourceId": "dataset-ioa-qa",
                    "fieldPath": "qa.hit_rate",
                    "evidenceIds": ["ev-qa-1", "ev-qa-2", "ev-qa-3"]
                },
                "visualization": {
                    "type": "line-chart",
                    "label": "命中趋势"
                },
                "layout": { "x": 4, "y": 0, "w": 4, "h": 3 }
            }, {
                "id": "topic-share",
                "title": "问题主题分布",
                "content": "高级图表使用安全 ECharts JSON 岛，同时最终 HTML 保留确定性 SVG 回退。",
                "dataBinding": {
                    "label": "主题分布",
                    "sourceId": "dataset-ioa-qa",
                    "fieldPath": "qa.topic_share",
                    "evidenceIds": ["ev-topic-1", "ev-topic-2", "ev-topic-3"]
                },
                "visualization": {
                    "type": "bar-chart",
                    "label": "主题分布柱图",
                    "chartRuntime": "echarts",
                    "chartOptions": {
                        "title": { "text": "主题分布" },
                        "xAxis": { "type": "category" },
                        "yAxis": { "type": "value" },
                        "series": [{
                            "type": "bar",
                            "data": [36, 24, 18]
                        }]
                    }
                },
                "layout": { "x": 8, "y": 0, "w": 4, "h": 3 }
            }],
            "dataSnapshot": {
                "source": "static-page-render-smoke-fixture",
                "module_bindings": [{
                    "moduleId": "overview",
                    "sampleData": [
                        { "label": "知识段落", "value": 128, "kind": "module_data" },
                        { "label": "权限命中", "value": 96, "kind": "module_data" }
                    ],
                    "dataQuality": "module_data",
                    "bindingQuality": {
                        "status": "confirmed",
                        "chartDataFit": "ready",
                        "reason": "renderable_data_rows"
                    }
                }, {
                    "moduleId": "answer-trend",
                    "sampleData": [
                        { "label": "第一轮", "value": 82, "kind": "evidence_value" },
                        { "label": "第二轮", "value": 89, "kind": "evidence_value" },
                        { "label": "第三轮", "value": 94, "kind": "evidence_value" }
                    ],
                    "dataQuality": "evidence_value",
                    "bindingQuality": {
                        "status": "confirmed",
                        "chartDataFit": "ready",
                        "reason": "renderable_data_rows"
                    }
                }, {
                    "moduleId": "topic-share",
                    "sampleData": [
                        { "label": "权限问题", "value": 36, "kind": "evidence_value" },
                        { "label": "流程问题", "value": 24, "kind": "evidence_value" },
                        { "label": "资料问题", "value": 18, "kind": "evidence_value" }
                    ],
                    "dataQuality": "evidence_value",
                    "bindingQuality": {
                        "status": "confirmed",
                        "chartDataFit": "ready",
                        "reason": "renderable_data_rows"
                    }
                }]
            }
        }),
        selected_scope: json!({
            "mode": "selected_datasets",
            "datasetIds": ["dataset-ioa-qa"]
        }),
        visibility_snapshot: json!({
            "policy": "assistant_run_scope_snapshot",
            "userRole": "dataset_tester"
        }),
        preview_asset_key: Some("previews/smoke-static-page.png".to_string()),
        image_job_id: Some("smoke-image-job".to_string()),
    };

    let result = render_static_page(&request);
    let summary = json!({
        "renderer": STATIC_PAGE_RENDERER_ID,
        "title": request.title,
        "module_count": result.asset_manifest["module_count"],
        "chart_runtime": result.asset_manifest["chart_runtime"],
        "export_package": result.asset_manifest["export_package"],
        "files": {
            "html": "index.html",
            "manifest": "asset-manifest.json",
            "request": "render-request.json",
            "summary": "smoke-summary.json"
        }
    });

    let data_quality_report = json!({
        "kind": "static-page-data-quality-report",
        "version": 1,
        "summary": result.asset_manifest["chart_runtime"]["dataQualitySummary"],
        "modules": result.asset_manifest["chart_runtime"]["modules"]
    });

    fs::write(output_dir.join("index.html"), &result.html)?;
    fs::write(
        output_dir.join("asset-manifest.json"),
        serde_json::to_string_pretty(&result.asset_manifest)?,
    )?;
    write_json_file(
        &output_dir,
        "export-package.json",
        &result.asset_manifest["export_package"],
    )?;
    write_json_file(
        &output_dir,
        "data-snapshot.json",
        &result.asset_manifest["data_snapshot"],
    )?;
    write_json_file(
        &output_dir,
        "data.json",
        &result.asset_manifest["data_snapshot"],
    )?;
    write_json_file(
        &output_dir,
        "data-quality-report.json",
        &data_quality_report,
    )?;
    write_json_file(
        &output_dir,
        "visual-bridge.json",
        &result.asset_manifest["visual_bridge"],
    )?;
    write_json_file(
        &output_dir,
        "modules.json",
        &result.asset_manifest["modules"],
    )?;
    write_json_file(
        &output_dir,
        "runtime-requirements.json",
        &result.asset_manifest["export_package"]["runtime_requirements"],
    )?;
    write_json_file(
        &output_dir,
        "render-spec.json",
        &result.asset_manifest["render_spec"],
    )?;
    fs::write(
        output_dir.join("README.md"),
        render_smoke_readme(&request, &result.asset_manifest),
    )?;
    fs::write(
        output_dir.join("render-request.json"),
        serde_json::to_string_pretty(&request)?,
    )?;
    fs::write(
        output_dir.join("smoke-summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;

    println!(
        "static page render smoke artifact: {}",
        output_dir.display()
    );
    Ok(())
}

fn write_json_file(
    output_dir: &Path,
    file_name: &str,
    value: &serde_json::Value,
) -> Result<(), Box<dyn Error>> {
    fs::write(
        output_dir.join(file_name),
        serde_json::to_string_pretty(value)?,
    )?;
    Ok(())
}

fn render_smoke_readme(request: &StaticPageRenderRequest, manifest: &serde_json::Value) -> String {
    let quality = &manifest["chart_runtime"]["dataQualitySummary"];
    let confirmed = quality["confirmedModules"].as_u64().unwrap_or(0);
    let partial = quality["partialModules"].as_u64().unwrap_or(0);
    let missing = quality["missingModules"].as_u64().unwrap_or(0);
    let echarts_requested = manifest["chart_runtime"]["echartsRequestedModules"]
        .as_u64()
        .unwrap_or(0);
    let visual_status = manifest["visual_bridge"]["status"]
        .as_str()
        .unwrap_or("unknown");
    let preview_asset = manifest["visual_bridge"]["previewAssetKey"]
        .as_str()
        .unwrap_or("no-preview");
    format!(
        concat!(
            "# {}\n\n",
            "这个目录是静态页 renderer smoke 生成的代表性交付包，用于验证最终 HTML、数据快照、模块质量报告和浏览器交付合同。\n\n",
            "- 草稿 ID：{}\n",
            "- 渲染器：{}\n",
            "- 数据质量：已确认 {} / 部分 {} / 缺失 {}\n",
            "- ECharts 可选增强模块：{}\n",
            "- 视觉合同：{} / {}\n",
            "- 浏览器交付：index.html 可直接打开；无远程脚本；图表保留 deterministic DOM/SVG 回退。\n",
            "- 动态数据入口：data.json（与 data-snapshot.json 同源，可由发布侧在资料更新后替换）。\n",
            "- 模块级数据质量报告：data-quality-report.json\n",
            "- 视觉合同桥：visual-bridge.json\n",
            "- 运行要求：runtime-requirements.json\n"
        ),
        request.title,
        request.draft_id,
        STATIC_PAGE_RENDERER_ID,
        confirmed,
        partial,
        missing,
        echarts_requested,
        visual_status,
        preview_asset,
    )
}
