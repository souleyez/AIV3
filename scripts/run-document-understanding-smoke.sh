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
run_check "PDF parser candidate selection scores structure and coverage" \
  "${cargo_bin}" test -p ingest-worker pdf_candidate_selection --lib
run_check "weak usable PDF text can trigger bounded VLM rescue" \
  "${cargo_bin}" test -p ingest-worker pdf_vlm_rescue --lib
run_check "ingest outcomes expose model-visible parse status" \
  "${cargo_bin}" test -p ingest-worker ingest_outcome_derives_model_visible_parse_status --lib
run_check "structure-aware chunks use PaddleOCR blocks" \
  "${cargo_bin}" test -p ingest-worker build_document_chunks_uses_paddleocr_structure_title_blocks
run_check "chunk understanding carries paragraphs and candidate terms" \
  "${cargo_bin}" test -p ingest-worker chunk_understanding_metadata_extracts_paragraphs_and_noun_terms
run_check "extracted document chunks preserve parser sections" \
  "${cargo_bin}" test -p ingest-worker split_extracted_document_chunks
run_check "low-quality ingest auto reparse is requeued" \
  "${cargo_bin}" test -p ingest-worker degraded_uploaded_document_is_failed_and_requeued_for_auto_reparse
run_check "external parse accepts Java camelCase payloads" \
  "${cargo_bin}" test -p contracts external_document_parse_request_accepts_java_camel_case_payload --lib
run_check "external parse-detail exposes Java-compatible summary fields" \
  "${cargo_bin}" test -p contracts external_document_parse_detail_response_exposes_java_compat_summary_fields --lib
run_check "external parse-detail surfaces async status and infers source" \
  "${cargo_bin}" test -p platform-api external_document_parse_endpoint_downloads_and_enqueues_ingest --lib
run_check "external parse auto-creates source dataset" \
  "${cargo_bin}" test -p platform-api external_document_parse_endpoint_auto_creates_source_dataset_when_missing --lib
run_check "external chat document scope infers source from documentExternalId" \
  "${cargo_bin}" test -p platform-api external_channel_document_scope_infers_source_from_document_external_id --lib
run_check "external chat missing document source is model-visible" \
  "${cargo_bin}" test -p platform-api external_channel_document_scope_missing_source_is_model_visible --lib
run_check "document detail exposes parse state" \
  "${cargo_bin}" test -p platform-api load_document_detail_returns_document_chunks_and_retrieval_evidences --lib
run_check "parse structure hints feed document chunk search" \
  "${cargo_bin}" test -p platform-api document_chunk_search_text_includes_parse_structure_hints --lib
run_check "parse quality diagnostics are compact and model-visible" \
  "${cargo_bin}" test -p platform-api assistant_run_document_parse_quality_summary_compacts_parser_diagnostics --lib
run_check "resume company scan expands supply actions" \
  "${cargo_bin}" test -p platform-api assistant_run_resume_company_scan_scope_expands_supply_actions --lib
run_check "general entity scan prompts request dataset scan" \
  "${cargo_bin}" test -p platform-api assistant_run_general_entity_scan_prompts_request_dataset_scan --lib
run_check "resume company extractor keeps valid company names" \
  "${cargo_bin}" test -p platform-api assistant_run_extracts_company_names --lib
run_check "resume company extractor filters phrase noise" \
  "${cargo_bin}" test -p platform-api assistant_run_filters_resume_company_phrase_noise --lib
run_check "resume company extractor normalizes group relation names" \
  "${cargo_bin}" test -p platform-api assistant_run_normalizes_group_relation_company_names --lib
run_check "typed document entities and candidate terms are extracted" \
  "${cargo_bin}" test -p platform-api assistant_run_extracts_typed_document_entities_and_candidate_terms --lib
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
    candidate_selection: "PDF parser candidates should be selected by text coverage plus structure/layout signals instead of returning the first minimally usable parser output.",
    vlm_rescue: "Short unstructured PDF text that barely passes the minimum quality gate may try MiniMax VLM rescue when configured; structured or complete local parses should not pay that cost.",
    structure_aware_chunks: "PaddleOCR title/table blocks should seed section title hints, paragraph samples, and candidate noun terms for chunk metadata and later model supply.",
    parser_section_chunks: "When parser section blocks are available, ingest chunking should preserve section boundaries before falling back to hard max-character splitting.",
    ingest_parse_status: "Ingest outcomes should persist model-visible parse status values such as parsed, parse_degraded, and parsed_with_vlm_fallback.",
    auto_reparse: "Low-quality ingest should fail the current task, persist auto_reparse metadata, and requeue the upload workflow instead of completing as usable content.",
    external_parse_compatibility: "Third-party Java/camelCase parse payloads and parse-detail summary fields remain compatible, including parseStatus/modelStatus/workflow state.",
    external_document_id_resolution: "Third-party documentExternalId should resolve parse-detail and chat document scope by inferring a unique source when source_id is omitted; unresolved IDs should remain model-visible instead of failing ordinary chat.",
    document_detail_parse_state: "Document detail responses should expose parse_state so normal UI/model flows can explain pending, failed, or reparsing documents.",
    resume_entity_scan: "Resume/company entity scans should preserve valid company names while filtering common phrase noise.",
    entity_scan_trigger: "Dataset questions asking to count, list, extract, or segment positions, skills, projects, locations, people, keywords, terms, or nouns should supply document entity scans.",
    typed_entity_terms: "Document entity scans should expose conservative typed entities and candidate noun terms beyond company names.",
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
  `- Candidate selection: ${report.contract.candidate_selection}`,
  `- VLM rescue: ${report.contract.vlm_rescue}`,
  `- Structure-aware chunks: ${report.contract.structure_aware_chunks}`,
  `- Parser section chunks: ${report.contract.parser_section_chunks}`,
  `- Ingest parse status: ${report.contract.ingest_parse_status}`,
  `- Auto reparse: ${report.contract.auto_reparse}`,
  `- External parse compatibility: ${report.contract.external_parse_compatibility}`,
  `- External document ID resolution: ${report.contract.external_document_id_resolution}`,
  `- Document detail parse state: ${report.contract.document_detail_parse_state}`,
  `- Resume entity scan: ${report.contract.resume_entity_scan}`,
  `- Entity scan trigger: ${report.contract.entity_scan_trigger}`,
  `- Typed entity terms: ${report.contract.typed_entity_terms}`,
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
