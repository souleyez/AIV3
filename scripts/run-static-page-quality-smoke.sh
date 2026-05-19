#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${STATIC_PAGE_QUALITY_SMOKE_REPORT_DIR:-${repo_root}/target/static-page-quality-smoke}"
report_basename="static-page-quality-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
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

echo "Static page quality smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Report directory: ${report_dir}"
echo "Cargo: ${cargo_bin}"

checks=()

run_check() {
  local name="$1"
  shift
  echo ""
  echo "== ${name} =="
  "$@"
  checks+=("${name}")
}

run_check "data snapshot extracts evidence field candidates" \
  "${cargo_bin}" test -p platform-api static_page_data_snapshot_extracts_field_candidates_from_evidence_state
run_check "data snapshot exposes media field candidates" \
  "${cargo_bin}" test -p platform-api static_page_data_snapshot_exposes_media_field_candidates
run_check "data snapshot prefers explicit evidence values" \
  "${cargo_bin}" test -p platform-api static_page_data_snapshot_prefers_explicit_evidence_values
run_check "data snapshot carries section title hints" \
  "${cargo_bin}" test -p platform-api static_page_data_snapshot_carries_section_title_hints
run_check "preview gate blocks unrenderable chart bindings" \
  "${cargo_bin}" test -p platform-api static_page_preview_gate_blocks_unrenderable_chart_bindings
run_check "preview gate blocks inferred evidence signals" \
  "${cargo_bin}" test -p platform-api static_page_preview_gate_blocks_inferred_evidence_signals
run_check "final render gate blocks unrenderable chart bindings" \
  "${cargo_bin}" test -p platform-api static_page_final_render_gate_blocks_unrenderable_chart_bindings
run_check "explicit module data survives preview and render snapshots" \
  "${cargo_bin}" test -p platform-api static_page_data_snapshot_preserves_module_explicit_data_for_preview_and_render
run_check "external direct HTML render exposes downloadable URL" \
  "${cargo_bin}" test -p platform-api external_channel_direct_html_render_can_be_downloaded_with_channel_token
run_check "external HTML download URL requires external scope" \
  "${cargo_bin}" test -p platform-api static_page_html_download_url_requires_external_channel_scope
run_check "Codex plan-only blocks preview when data quality needs attention" \
  "${cargo_bin}" test -p assistant-runtime codex_executor_plan_only_blocks_preview_when_static_page_data_quality_needs_attention
run_check "Codex plan-only allows preview when data quality is confirmed" \
  "${cargo_bin}" test -p assistant-runtime codex_executor_plan_only_allows_preview_when_static_page_data_quality_is_confirmed

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
checks_json="$(
  printf '%s\n' "${checks[@]}" | node -e '
const fs = require("fs");
const checks = fs.readFileSync(0, "utf8").trim().split(/\n/).filter(Boolean);
process.stdout.write(JSON.stringify(checks.map((name) => ({ name, status: "passed" }))));
'
)"

SMOKE_REPO_ROOT="${repo_root}" \
SMOKE_HEAD="${head_short}" \
SMOKE_STARTED_AT="${started_at}" \
SMOKE_FINISHED_AT="${finished_at}" \
SMOKE_CHECKS_JSON="${checks_json}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const report = {
  smoke: "static-page-quality",
  ready: true,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  contract: {
    data_snapshot: "Static pages carry field candidates, explicit evidence values, media windows, module sample data, and section-title hints into the draft data snapshot.",
    preview_gate: "Effect-image preview is blocked when chart modules lack renderable sample rows or only have inferred evidence signals.",
    final_render_gate: "Final static-page render is blocked when chart modules still lack renderable sample rows.",
    codex_plan_only: "Codex plan-only suggestions must repair weak static-page data quality before submitting preview generation."
  },
  checks: JSON.parse(process.env.SMOKE_CHECKS_JSON || "[]"),
  notes: [
    "Default smoke is non-destructive and does not load /etc/aiv3/aiv3.env.",
    "This smoke validates the static-page data-quality contract before effect-preview or final-render work.",
    "Visual/browser rendering inspection remains a separate smoke for generated HTML artifacts."
  ]
};
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# Static Page Quality Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  "",
  "## Contract",
  "",
  `- Data snapshot: ${report.contract.data_snapshot}`,
  `- Preview gate: ${report.contract.preview_gate}`,
  `- Final render gate: ${report.contract.final_render_gate}`,
  `- Codex plan-only: ${report.contract.codex_plan_only}`,
  "",
  "## Checks",
  "",
  ...report.checks.map((check) => `- ${check.status}: \`${check.name}\``),
  "",
  "## Notes",
  "",
  ...report.notes.map((note) => `- ${note}`),
  "",
].join("\n");
fs.writeFileSync(1, lines);
NODE

echo ""
echo "Static page quality smoke report: ${report_json}"
echo "Static page quality smoke summary: ${report_md}"
echo "OK static-page-quality smoke completed."
