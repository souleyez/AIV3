#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${ASSISTANT_RUN_WORKER_SMOKE_REPORT_DIR:-${repo_root}/target/assistant-run-worker-smoke}"
report_basename="assistant-run-worker-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
database_test="${ASSISTANT_RUN_WORKER_SMOKE_DATABASE_TEST:-false}"
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

echo "AssistantRun worker smoke started"
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

run_check "cargo check -p assistant-run-worker" \
  "${cargo_bin}" check -p assistant-run-worker
run_check "cargo test -p assistant-run-worker" \
  "${cargo_bin}" test -p assistant-run-worker
run_check "cargo test -p workflow-definitions assistant_run_model_completion_starts_on_dedicated_queue" \
  "${cargo_bin}" test -p workflow-definitions assistant_run_model_completion_starts_on_dedicated_queue
run_check "cargo test -p domain-model workflow_kind_roundtrips_video_extraction" \
  "${cargo_bin}" test -p domain-model workflow_kind_roundtrips_video_extraction
run_check "cargo test -p media-worker model_completion" \
  "${cargo_bin}" test -p media-worker model_completion

database_check_status="skipped"
database_check_reason="set ASSISTANT_RUN_WORKER_SMOKE_DATABASE_TEST=true with a disposable PLATFORM_DATABASE_URL to run the DB-backed consumer check"
if [[ "${database_test}" == "true" ]]; then
  if [[ -z "${PLATFORM_DATABASE_URL:-}" ]]; then
    echo "PLATFORM_DATABASE_URL is required when ASSISTANT_RUN_WORKER_SMOKE_DATABASE_TEST=true." >&2
    exit 1
  fi
  if [[ "${PLATFORM_DATABASE_URL}" == *"/ai_data_platform_v3"* && "${PLATFORM_DATABASE_URL}" != *"test"* ]]; then
    echo "Refusing DB-backed smoke against a non-test ai_data_platform_v3 database." >&2
    echo "Point PLATFORM_DATABASE_URL at a disposable test database." >&2
    exit 1
  fi
  run_check "cargo test -p platform-api assistant_run_model_completion_dispatch_consumes_continue_once" \
    "${cargo_bin}" test -p platform-api assistant_run_model_completion_dispatch_consumes_continue_once
  database_check_status="passed"
  database_check_reason=""
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
SMOKE_CHECKS_JSON="${checks_json}" \
SMOKE_DATABASE_CHECK_STATUS="${database_check_status}" \
SMOKE_DATABASE_CHECK_REASON="${database_check_reason}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const report = {
  smoke: "assistant-run-worker",
  ready: true,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  queue: "assistant_run",
  task_key: "consume_model_completion_turn",
  workflow_kind: "assistant_run_model_completion_workflow",
  checks: JSON.parse(process.env.SMOKE_CHECKS_JSON || "[]"),
  database_backed_consumer_check: {
    status: process.env.SMOKE_DATABASE_CHECK_STATUS,
    reason: process.env.SMOKE_DATABASE_CHECK_REASON
  },
  notes: [
    "Default smoke is non-destructive and does not load /etc/aiv3/aiv3.env.",
    "Run the DB-backed consumer check only against a disposable test database.",
    "Production service enablement still requires the process manager to run cargo run -p assistant-run-worker or the built assistant-run-worker binary."
  ]
};
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# AssistantRun Worker Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  `- Queue: ${report.queue}`,
  `- Task key: ${report.task_key}`,
  `- Workflow kind: ${report.workflow_kind}`,
  "",
  "## Checks",
  "",
  ...report.checks.map((check) => `- ${check.status}: \`${check.name}\``),
  "",
  "## DB-Backed Consumer Check",
  "",
  `- Status: ${report.database_backed_consumer_check.status}`,
  report.database_backed_consumer_check.reason
    ? `- Reason: ${report.database_backed_consumer_check.reason}`
    : "",
  "",
  "## Notes",
  "",
  ...report.notes.map((note) => `- ${note}`),
  "",
].filter((line) => line !== null).join("\n");
fs.writeFileSync(1, lines);
NODE

echo ""
echo "AssistantRun worker smoke report: ${report_json}"
echo "AssistantRun worker smoke summary: ${report_md}"
echo "OK assistant-run-worker smoke completed."
