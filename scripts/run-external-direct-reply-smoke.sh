#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${EXTERNAL_DIRECT_REPLY_SMOKE_REPORT_DIR:-${repo_root}/target/external-direct-reply-smoke}"
report_basename="external-direct-reply-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
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

echo "External direct reply smoke started"
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

run_check "ordinary external chat returns provider-authored answered text" \
  "${cargo_bin}" test -p platform-api generic_chat_page_event_returns_provider_model_text_when_configured --lib
run_check "ordinary external chat falls back when primary output is rejected" \
  "${cargo_bin}" test -p platform-api generic_chat_page_event_uses_fallback_when_primary_output_is_rejected --lib
run_check "external chat stream emits started/delta/completed without accepted" \
  "${cargo_bin}" test -p platform-api generic_chat_page_event_stream_does_not_emit_accepted_as_answer_state --lib
run_check "missing model config does not return accepted text" \
  "${cargo_bin}" test -p platform-api generic_chat_page_event_without_model_returns_unavailable_error_not_accepted_reply --lib
run_check "plain status prompts do not enter action planning" \
  "${cargo_bin}" test -p platform-api external_channel_action_planning_gates_plain_questions --lib
run_check "OpenAI-compatible provider timeout is enforced" \
  "${cargo_bin}" test -p llm-gateway openai_compatible_provider_applies_configured_timeout --lib
run_check "one-character PDF extraction is low quality" \
  "${cargo_bin}" test -p ingest-worker pdf_parse_quality_marks_one_character_extract_as_low_coverage --lib
run_check "low-quality PDF diagnostic blocks single-character success" \
  "${cargo_bin}" test -p ingest-worker pdf_low_quality_diagnostic_does_not_treat_single_character_as_content --lib

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
  smoke: "external-direct-reply",
  ready: true,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  contract: {
    ordinary_external_chat: "Final user-visible ordinary chat replies must be provider-authored text with task_status=answered.",
    no_orchestration_answer: "accepted, duplicate_accepted, model_unavailable, and model_output_suppressed are not valid ordinary-chat answers.",
    fallback: "Timeouts, provider failures, empty output, and unsafe/internal output are retryable before any final response is returned.",
    pdf_quality: "One-character or otherwise too-short PDF extraction is low quality and must trigger OCR/VLM fallback or a parse-quality diagnostic."
  },
  checks: JSON.parse(process.env.SMOKE_CHECKS_JSON || "[]")
};
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# External Direct Reply Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  "",
  "## Contract",
  "",
  `- Ordinary external chat: ${report.contract.ordinary_external_chat}`,
  `- No orchestration answer: ${report.contract.no_orchestration_answer}`,
  `- Fallback: ${report.contract.fallback}`,
  `- PDF quality: ${report.contract.pdf_quality}`,
  "",
  "## Checks",
  "",
  ...report.checks.map((check) => `- ${check.status}: \`${check.name}\``),
  "",
].join("\n");
fs.writeFileSync(1, lines);
NODE

echo ""
echo "External direct reply smoke report: ${report_json}"
echo "External direct reply smoke summary: ${report_md}"
echo "OK external-direct-reply smoke completed."
