#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${NEWBAI_ASSISTANT_ANSWER_SMOKE_REPORT_DIR:-${repo_root}/target/newbai-assistant-answer-smoke}"
report_basename="newbai-assistant-answer-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
checks_jsonl="${report_dir}/${report_basename}.checks.jsonl"
cargo_bin="${CARGO_BIN:-cargo}"
require_db="${NEWBAI_ASSISTANT_ANSWER_SMOKE_REQUIRE_DB:-false}"

if ! command -v "${cargo_bin}" >/dev/null 2>&1; then
  for candidate in "${HOME:-}/.cargo/bin/cargo" "/opt/homebrew/bin/cargo" "/usr/local/bin/cargo"; do
    if [[ -x "${candidate}" ]]; then
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
: >"${checks_jsonl}"

head_short="$(git rev-parse --short HEAD)"
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
db_fixture_status="not_run"

echo "NewBai assistant answer smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Report directory: ${report_dir}"
echo "Cargo: ${cargo_bin}"

run_check() {
  local key="$1"
  local name="$2"
  shift 2
  local log_path="${report_dir}/${report_basename}-${key}.log"
  local status="passed"
  local fixture_status="not_applicable"
  local exit_code=0

  echo ""
  echo "== ${name} =="
  set +e
  "$@" 2>&1 | tee "${log_path}"
  exit_code="${PIPESTATUS[0]}"
  set -e

  if [[ "${exit_code}" -ne 0 ]]; then
    status="failed"
  fi
  if grep -q "skipping NewBai assistant answer supply smoke:" "${log_path}"; then
    status="skipped_fixture"
    fixture_status="skipped_protected_database"
    db_fixture_status="${fixture_status}"
  elif [[ "${key}" == "assistant_run_route" ]]; then
    fixture_status="executed"
    db_fixture_status="${fixture_status}"
  fi

  CHECKS_JSONL="${checks_jsonl}" \
  CHECK_KEY="${key}" \
  CHECK_NAME="${name}" \
  CHECK_STATUS="${status}" \
  CHECK_EXIT_CODE="${exit_code}" \
  CHECK_FIXTURE_STATUS="${fixture_status}" \
  CHECK_LOG_PATH="${log_path}" \
  node <<'NODE'
const fs = require("fs");
const entry = {
  key: process.env.CHECK_KEY,
  name: process.env.CHECK_NAME,
  status: process.env.CHECK_STATUS,
  exit_code: Number(process.env.CHECK_EXIT_CODE || 0),
  fixture_status: process.env.CHECK_FIXTURE_STATUS,
  log_path: process.env.CHECK_LOG_PATH,
};
fs.appendFileSync(process.env.CHECKS_JSONL, JSON.stringify(entry) + "\n");
NODE

  if [[ "${exit_code}" -ne 0 ]]; then
    echo "check failed: ${name}" >&2
    exit "${exit_code}"
  fi
}

run_check "provider_input_contract" \
  "NewBai model-facing evidence contains answerable business tokens" \
  "${cargo_bin}" test -p platform-api \
  assistant_run_newbai_provider_input_contains_answerable_evidence_without_db_fixture \
  --lib

run_check "assistant_run_route" \
  "NewBai AssistantRun route surfaces ranked business evidence" \
  "${cargo_bin}" test -p platform-api \
  assistant_run_newbai_answer_supply_surfaces_ranked_business_evidence \
  --lib -- --nocapture

run_check "retrieval_ranking_regression" \
  "NewBai retrieval ranking regressions remain green" \
  "${cargo_bin}" test -p platform-api retrieval_ranking --lib

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

SMOKE_REPO_ROOT="${repo_root}" \
SMOKE_HEAD="${head_short}" \
SMOKE_STARTED_AT="${started_at}" \
SMOKE_FINISHED_AT="${finished_at}" \
SMOKE_CHECKS_JSONL="${checks_jsonl}" \
SMOKE_DB_FIXTURE_STATUS="${db_fixture_status}" \
SMOKE_REQUIRE_DB="${require_db}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const checks = fs
  .readFileSync(process.env.SMOKE_CHECKS_JSONL, "utf8")
  .trim()
  .split(/\n/)
  .filter(Boolean)
  .map((line) => JSON.parse(line));
const failed = checks.filter((check) => check.status === "failed");
const skipped = checks.filter((check) => check.status === "skipped_fixture");
const dbFixtureStatus = process.env.SMOKE_DB_FIXTURE_STATUS || "not_run";
const requireDb = /^(1|true|yes|on)$/i.test(process.env.SMOKE_REQUIRE_DB || "");
const assistantRunRouteValidated = dbFixtureStatus === "executed";
const report = {
  smoke: "newbai-assistant-answer",
  command_succeeded: failed.length === 0 && (!requireDb || assistantRunRouteValidated),
  ready: failed.length === 0 && assistantRunRouteValidated,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  db_fixture_status: dbFixtureStatus,
  assistant_run_route_validated: assistantRunRouteValidated,
  model_input_contract_validated: checks.some(
    (check) => check.key === "provider_input_contract" && check.status === "passed",
  ),
  retrieval_ranking_validated: checks.some(
    (check) => check.key === "retrieval_ranking_regression" && check.status === "passed",
  ),
  require_db: requireDb,
  checks,
  notes: [
    "This smoke does not enable RETRIEVAL_SEARCH_BACKEND=postgres_lexical and does not call a real LLM provider.",
    "The provider-input contract is deterministic and validates that NewBai business evidence is present in model-facing supply.",
    "The AssistantRun route check uses the local Postgres fixture. It is considered route-validated only when PLATFORM_DATABASE_URL points to a disposable test database, or an explicit fixture override is intentionally set.",
    "A skipped DB fixture protects shared databases from truncate/reset and must not be treated as full customer-answer live evidence.",
  ],
};
if (skipped.length > 0 && !requireDb) {
  report.notes.push(
    "One or more fixture-backed checks were skipped by the disposable-database safety guard.",
  );
}
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# NewBai Assistant Answer Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "not fully validated"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  `- DB fixture status: ${report.db_fixture_status}`,
  `- AssistantRun route validated: ${report.assistant_run_route_validated}`,
  `- Provider input contract validated: ${report.model_input_contract_validated}`,
  `- Retrieval ranking validated: ${report.retrieval_ranking_validated}`,
  "",
  "## Checks",
  "",
  ...report.checks.map(
    (check) =>
      `- ${check.status}: \`${check.name}\` (${check.fixture_status}; log: ${check.log_path})`,
  ),
  "",
  "## Notes",
  "",
  ...report.notes.map((note) => `- ${note}`),
  "",
].join("\n");
fs.writeFileSync(1, lines);
NODE

echo ""
echo "NewBai assistant answer smoke report: ${report_json}"
echo "NewBai assistant answer smoke summary: ${report_md}"

if [[ "${require_db}" =~ ^([Tt][Rr][Uu][Ee]|1|[Yy][Ee][Ss]|[Oo][Nn])$ ]] \
  && [[ "${db_fixture_status}" != "executed" ]]; then
  echo "NEWBAI_ASSISTANT_ANSWER_SMOKE_REQUIRE_DB=true but AssistantRun DB fixture was not executed." >&2
  exit 1
fi

echo "OK newbai-assistant-answer smoke completed."
