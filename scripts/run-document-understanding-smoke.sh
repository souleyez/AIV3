#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${DOCUMENT_UNDERSTANDING_SMOKE_REPORT_DIR:-${repo_root}/target/document-understanding-smoke}"
report_basename="document-understanding-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
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

echo "Document understanding smoke started"
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

run_check "PaddleOCR parser contract tests" \
  "${cargo_bin}" test -p ingest-worker paddleocr --lib
run_check "one-character PDF extraction is low quality" \
  "${cargo_bin}" test -p ingest-worker pdf_parse_quality_marks_one_character_extract_as_low_coverage --lib
run_check "low-quality PDF diagnostic blocks single-character success" \
  "${cargo_bin}" test -p ingest-worker pdf_low_quality_diagnostic_does_not_treat_single_character_as_content --lib
run_check "ingest outcomes expose model-visible parse status" \
  "${cargo_bin}" test -p ingest-worker ingest_outcome_derives_model_visible_parse_status --lib
run_check "low-quality ingest auto reparse is requeued" \
  "${cargo_bin}" test -p ingest-worker degraded_uploaded_document_is_failed_and_requeued_for_auto_reparse
run_check "external parse accepts Java camelCase payloads" \
  "${cargo_bin}" test -p contracts external_document_parse_request_accepts_java_camel_case_payload --lib
run_check "external parse-detail exposes Java-compatible summary fields" \
  "${cargo_bin}" test -p contracts external_document_parse_detail_response_exposes_java_compat_summary_fields --lib
run_check "external parse-detail surfaces async parse status" \
  "${cargo_bin}" test -p platform-api external_document_parse_endpoint_downloads_and_enqueues_ingest --lib
run_check "document detail exposes parse state" \
  "${cargo_bin}" test -p platform-api load_document_detail_returns_document_chunks_and_retrieval_evidences --lib
run_check "resume company scan expands supply actions" \
  "${cargo_bin}" test -p platform-api assistant_run_resume_company_scan_scope_expands_supply_actions --lib
run_check "resume company extractor keeps valid company names" \
  "${cargo_bin}" test -p platform-api assistant_run_extracts_company_names --lib
run_check "resume company extractor filters phrase noise" \
  "${cargo_bin}" test -p platform-api assistant_run_filters_resume_company_phrase_noise --lib
run_check "resume company extractor normalizes group relation names" \
  "${cargo_bin}" test -p platform-api assistant_run_normalizes_group_relation_company_names --lib
run_check "async document parse status is supplied to assistant" \
  "${cargo_bin}" test -p platform-api assistant_run_supplies_document_parse_status_for_failed_and_reparsing_documents --lib

if [[ "${DOCUMENT_UNDERSTANDING_SMOKE_RUNTIME_GATE:-false}" =~ ^([Tt][Rr][Uu][Ee]|1|[Yy][Ee][Ss]|[Oo][Nn])$ ]]; then
  run_check "deployment ingest runtime gate" \
    bash scripts/run-ingest-runtime-gate.sh
fi

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
SMOKE_RUNTIME_GATE="${DOCUMENT_UNDERSTANDING_SMOKE_RUNTIME_GATE:-false}" \
SMOKE_CHECKS_JSON="${checks_json}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const report = {
  smoke: "document-understanding",
  ready: true,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  runtime_gate_requested: /^(true|1|yes|on)$/i.test(process.env.SMOKE_RUNTIME_GATE || ""),
  contract: {
    parser_quality: "PaddleOCR parsing, low-quality PDF detection, and diagnostic fallback must not treat one-character extraction as usable content.",
    ingest_parse_status: "Ingest outcomes should persist model-visible parse status values such as parsed, parse_degraded, and parsed_with_vlm_fallback.",
    auto_reparse: "Low-quality ingest should fail the current task, persist auto_reparse metadata, and requeue the upload workflow instead of completing as usable content.",
    external_parse_compatibility: "Third-party Java/camelCase parse payloads and parse-detail summary fields remain compatible, including parseStatus/modelStatus/workflow state.",
    document_detail_parse_state: "Document detail responses should expose parse_state so normal UI/model flows can explain pending, failed, or reparsing documents.",
    resume_entity_scan: "Resume/company entity scans should preserve valid company names while filtering common phrase noise.",
    async_parse_status: "Assistant evidence should surface failed, reparsing, and degraded document parse states so models can answer availability correctly."
  },
  checks: JSON.parse(process.env.SMOKE_CHECKS_JSON || "[]")
};
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# Document Understanding Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  `- Runtime gate requested: ${report.runtime_gate_requested}`,
  "",
  "## Contract",
  "",
  `- Parser quality: ${report.contract.parser_quality}`,
  `- Ingest parse status: ${report.contract.ingest_parse_status}`,
  `- Auto reparse: ${report.contract.auto_reparse}`,
  `- External parse compatibility: ${report.contract.external_parse_compatibility}`,
  `- Document detail parse state: ${report.contract.document_detail_parse_state}`,
  `- Resume entity scan: ${report.contract.resume_entity_scan}`,
  `- Async parse status: ${report.contract.async_parse_status}`,
  "",
  "## Checks",
  "",
  ...report.checks.map((check) => `- ${check.status}: \`${check.name}\``),
  "",
].join("\n");
fs.writeFileSync(1, lines);
NODE

echo ""
echo "Document understanding smoke report: ${report_json}"
echo "Document understanding smoke summary: ${report_md}"
echo "OK document-understanding smoke completed."
