use serde_json::json;
use static_page_renderer::{render_static_page, StaticPageRenderRequest, STATIC_PAGE_RENDERER_ID};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let request_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: render_static_page_request <request.json> <output-dir>")?;
    let output_dir = env::args()
        .nth(2)
        .map(PathBuf::from)
        .ok_or("usage: render_static_page_request <request.json> <output-dir>")?;
    fs::create_dir_all(&output_dir)?;

    let request: StaticPageRenderRequest =
        serde_json::from_str(&fs::read_to_string(&request_path)?)?;
    let result = render_static_page(&request);
    let data_quality_report = json!({
        "kind": "static-page-data-quality-report",
        "version": 1,
        "summary": result.asset_manifest["chart_runtime"]["dataQualitySummary"],
        "modules": result.asset_manifest["chart_runtime"]["modules"],
    });
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
            "summary": "render-summary.json"
        }
    });

    fs::write(output_dir.join("index.html"), &result.html)?;
    write_json_file(&output_dir, "asset-manifest.json", &result.asset_manifest)?;
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
        render_readme(&request, &result.asset_manifest),
    )?;
    fs::write(
        output_dir.join("render-request.json"),
        serde_json::to_string_pretty(&request)?,
    )?;
    fs::write(
        output_dir.join("render-summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;

    println!("static page render artifact: {}", output_dir.display());
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

fn render_readme(request: &StaticPageRenderRequest, manifest: &serde_json::Value) -> String {
    let quality = &manifest["chart_runtime"]["dataQualitySummary"];
    let confirmed = quality["confirmedModules"].as_u64().unwrap_or(0);
    let partial = quality["partialModules"].as_u64().unwrap_or(0);
    let missing = quality["missingModules"].as_u64().unwrap_or(0);
    let visual_status = manifest["visual_bridge"]["status"]
        .as_str()
        .unwrap_or("unknown");
    format!(
        concat!(
            "# {}\n\n",
            "这个目录由 static-page-renderer-v1 从结构化 StaticPageRenderRequest 生成。\n\n",
            "- 草稿 ID：{}\n",
            "- 渲染器：{}\n",
            "- 数据质量：已确认 {} / 部分 {} / 缺失 {}\n",
            "- 视觉合同：{}\n",
            "- 入口：index.html\n",
            "- Manifest：asset-manifest.json\n",
            "- 数据快照：data-snapshot.json\n",
            "- 动态数据入口：data.json\n",
            "- 质量报告：data-quality-report.json\n",
            "- 视觉桥接：visual-bridge.json\n",
            "- 运行要求：runtime-requirements.json\n"
        ),
        request.title,
        request.draft_id,
        STATIC_PAGE_RENDERER_ID,
        confirmed,
        partial,
        missing,
        visual_status,
    )
}
