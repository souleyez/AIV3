#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${ASSISTANT_CHAT_CONTRACT_SMOKE_REPORT_DIR:-${repo_root}/target/assistant-chat-contract-smoke}"
report_basename="assistant-chat-contract-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
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

echo "Assistant chat contract smoke started"
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

run_check "ordinary chat remains unrestricted and V3-aware" \
  "${cargo_bin}" test -p platform-api assistant_run_provider_input_keeps_plain_ordinary_chat_unrestricted
run_check "continue chat remains unrestricted and V3-aware" \
  "${cargo_bin}" test -p platform-api assistant_run_continue_provider_input_keeps_plain_chat_unrestricted_and_v3_aware
run_check "ReAct invalid output only keeps safe natural direct answers" \
  "${cargo_bin}" test -p platform-api assistant_run_react_invalid_output_keeps_only_natural_direct_answers
run_check "ReAct natural fallback prompt blocks internal payloads" \
  "${cargo_bin}" test -p platform-api assistant_run_react_natural_fallback_prompt_blocks_internal_payloads
run_check "ReAct final answers reject raw observation payloads" \
  "${cargo_bin}" test -p platform-api final_answer_result_rejects_raw_observation_payloads
run_check "ReAct final answers reject fenced raw observation payloads" \
  "${cargo_bin}" test -p platform-api final_answer_result_rejects_fenced_raw_observation_payloads
run_check "external task callbacks hide internal observability fields" \
  "${cargo_bin}" test -p platform-api external_platform_task_status_callback_hides_internal_observability_fields
run_check "external action/search callbacks hide internal observability fields" \
  "${cargo_bin}" test -p platform-api external_channel_action_and_search_replies_hide_internal_observability_fields

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
  smoke: "assistant-chat-contract",
  ready: true,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  contract: {
    ordinary_chat: "V3 context is additive; plain chat can use general model ability.",
    evidence_boundary: "V3 facts require supplied evidence; unavailable V3 evidence must be called out as currently invisible or unsupplied.",
    user_answer_surface: "Assistant-visible answers must be natural language and must not expose observation, trace, runtime manifest, provider payloads, or execution trail fields.",
    external_channel_surface: "External callbacks carry task/action/search status only through public redacted fields."
  },
  checks: JSON.parse(process.env.SMOKE_CHECKS_JSON || "[]"),
  notes: [
    "Default smoke is non-destructive and does not load /etc/aiv3/aiv3.env.",
    "This smoke validates the user-facing answer contract and callback redaction shape.",
    "Run deployment-target model smokes separately when real provider credentials are intentionally enabled."
  ]
};
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# Assistant Chat Contract Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  "",
  "## Contract",
  "",
  `- Ordinary chat: ${report.contract.ordinary_chat}`,
  `- Evidence boundary: ${report.contract.evidence_boundary}`,
  `- User answer surface: ${report.contract.user_answer_surface}`,
  `- External channel surface: ${report.contract.external_channel_surface}`,
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
echo "Assistant chat contract smoke report: ${report_json}"
echo "Assistant chat contract smoke summary: ${report_md}"
echo "OK assistant-chat-contract smoke completed."
