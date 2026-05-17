#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${STATIC_PAGE_RENDER_SMOKE_REPORT_DIR:-${repo_root}/target/static-page-render-smoke}"
report_basename="static-page-render-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
artifact_dir="${report_dir}/${report_basename}-artifact"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
cargo_bin="${CARGO_BIN:-cargo}"

if ! command -v "${cargo_bin}" >/dev/null 2>&1; then
  for candidate in \
    "${HOME:-}/.cargo/bin/cargo" \
    "${HOME:-}/.cargo/bin/cargo.exe" \
    "/mnt/c/Users/${USER:-}/.cargo/bin/cargo.exe"; do
    if [[ -n "${candidate}" && -x "${candidate}" ]]; then
      cargo_bin="${candidate}"
      break
    fi
  done
fi

if ! command -v "${cargo_bin}" >/dev/null 2>&1 && [[ ! -x "${cargo_bin}" ]]; then
  echo "cargo was not found. Install Rust or set CARGO_BIN=/path/to/cargo." >&2
  exit 1
fi

mkdir -p "${report_dir}"

head_short="$(git rev-parse --short HEAD)"
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "Static page render smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Artifact directory: ${artifact_dir}"
echo "Report directory: ${report_dir}"
echo "Cargo: ${cargo_bin}"

"${cargo_bin}" run -p static-page-renderer --example static_page_render_smoke -- "${artifact_dir}"

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

SMOKE_REPO_ROOT="${repo_root}" \
SMOKE_HEAD="${head_short}" \
SMOKE_STARTED_AT="${started_at}" \
SMOKE_FINISHED_AT="${finished_at}" \
SMOKE_ARTIFACT_DIR="${artifact_dir}" \
SMOKE_REPORT_JSON="${report_json}" \
SMOKE_REPORT_MD="${report_md}" \
node <<'NODE'
const fs = require("fs");
const path = require("path");

const artifactDir = process.env.SMOKE_ARTIFACT_DIR;
const htmlPath = path.join(artifactDir, "index.html");
const manifestPath = path.join(artifactDir, "asset-manifest.json");
const summaryPath = path.join(artifactDir, "smoke-summary.json");
const html = fs.readFileSync(htmlPath, "utf8");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const summary = JSON.parse(fs.readFileSync(summaryPath, "utf8"));
const checks = [];

function hasExportFile(filePath) {
  return (manifest.export_package?.files || []).some((file) => file.path === filePath);
}

function check(name, passed, details) {
  checks.push({
    name,
    status: passed ? "passed" : "failed",
    details,
  });
}

check(
  "generated HTML shell is complete",
  html.startsWith("<!doctype html>") &&
    html.includes('<meta name="viewport" content="width=device-width, initial-scale=1">') &&
    html.includes('class="module-grid"') &&
    html.includes("@media(max-width:720px)"),
  "DOCTYPE, viewport, module grid, and mobile media rule are present."
);
check(
  "all expected modules render as DOM sections",
  (html.match(/class="module"/g) || []).length === 3 &&
    html.includes("问答服务总览") &&
    html.includes("最近问答命中趋势") &&
    html.includes("问题主题分布"),
  "The smoke fixture renders three named modules."
);
check(
  "deterministic chart fallback renders as SVG",
  html.includes('<svg class="chart-svg line-chart"') &&
    html.includes('<svg class="chart-svg bar-chart"'),
  "Line and bar chart fallbacks are real SVG nodes."
);
check(
  "ECharts advanced chart uses safe JSON island",
  html.includes('class="static-page-echarts-option"') &&
    html.includes("echarts-hydration-target") &&
    html.includes("window.__staticPageHydrateEcharts") &&
    !/<script[^>]+\bsrc=/i.test(html),
  "Advanced chart hydration is inline JSON plus optional host-provided ECharts, with no remote script src."
);
check(
  "confirmed data does not show missing-data placeholder",
  !html.includes("数据待确认") &&
    !html.includes("等待确认图表数据") &&
    html.includes("第三轮: 94"),
  "Confirmed sample rows are rendered and missing-data labels stay absent."
);
check(
  "renderer manifest records final HTML contract",
  manifest.renderer === "static-page-renderer-v1" &&
    manifest.design_contract?.final_role === "html_css_svg_renderer" &&
    manifest.module_count === 3,
  "Renderer id, design contract, and module count are stable."
);
check(
  "chart runtime manifest is delivery-ready",
  manifest.chart_runtime?.dataQualitySummary?.attentionModules === 0 &&
    manifest.chart_runtime?.echartsRequestedModules === 1 &&
    manifest.chart_runtime?.echartsHydratableModules === 1 &&
    manifest.chart_runtime?.fallbackModules === 1,
  "All modules have confirmed data; the ECharts module also has deterministic fallback."
);
check(
  "export package lists handoff artifacts",
  manifest.export_package?.status === "rendered" &&
    hasExportFile("index.html") &&
    hasExportFile("asset-manifest.json") &&
    hasExportFile("data-quality-report.json") &&
    hasExportFile("visual-bridge.json") &&
    hasExportFile("runtime-requirements.json") &&
    hasExportFile("README.md"),
  "The render manifest exposes the expected export package files."
);
check(
  "export package records browser delivery contract",
  manifest.export_package?.browser_delivery_contract?.entry === "index.html" &&
    manifest.export_package?.browser_delivery_contract?.remote_scripts_allowed === false &&
    manifest.export_package?.browser_delivery_contract?.deterministic_chart_fallback === true &&
    manifest.export_package?.browser_delivery_contract?.optional_echarts_hydration === "safe_json_option_islands" &&
    manifest.export_package?.browser_delivery_contract?.mobile_viewport === "responsive_no_horizontal_overflow_expected",
  "The export manifest keeps the direct-browser handoff rule: no remote scripts, deterministic chart fallback, and optional safe ECharts hydration."
);
check(
  "generated artifact summary matches manifest",
  summary.renderer === manifest.renderer &&
    summary.module_count === manifest.module_count &&
    summary.chart_runtime?.dataQualitySummary?.attentionModules === 0,
  "Smoke summary mirrors the generated asset manifest."
);

const ready = checks.every((item) => item.status === "passed");
const report = {
  smoke: "static-page-render",
  ready,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  artifact_dir: artifactDir,
  artifacts: {
    html: htmlPath,
    manifest: manifestPath,
    summary: summaryPath,
  },
  contract: {
    generated_html: "Renderer smoke generates an actual index.html artifact from a representative static-page draft.",
    chart_runtime: "Deterministic SVG chart output remains available, and ECharts modules expose only safe JSON hydration islands without remote scripts.",
    data_quality: "Confirmed sample rows render without missing-data placeholders; manifest attentionModules must be zero for this fixture.",
    export_handoff: "The asset manifest must list the expected static-page export package files and preserve the browser delivery contract."
  },
  checks,
};

fs.writeFileSync(process.env.SMOKE_REPORT_JSON, JSON.stringify(report, null, 2));

const lines = [
  "# Static Page Render Smoke",
  "",
  `- Status: ${ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  `- Artifact directory: ${report.artifact_dir}`,
  "",
  "## Contract",
  "",
  `- Generated HTML: ${report.contract.generated_html}`,
  `- Chart runtime: ${report.contract.chart_runtime}`,
  `- Data quality: ${report.contract.data_quality}`,
  `- Export handoff: ${report.contract.export_handoff}`,
  "",
  "## Checks",
  "",
  ...checks.map((item) => `- ${item.status}: \`${item.name}\` - ${item.details}`),
  "",
].join("\n");
fs.writeFileSync(process.env.SMOKE_REPORT_MD, lines);

if (!ready) {
  console.error(`Static page render smoke failed. Report: ${process.env.SMOKE_REPORT_JSON}`);
  process.exit(1);
}
NODE

echo ""
echo "Static page render smoke artifact: ${artifact_dir}"
echo "Static page render smoke report: ${report_json}"
echo "Static page render smoke summary: ${report_md}"
echo "OK static-page-render smoke completed."
