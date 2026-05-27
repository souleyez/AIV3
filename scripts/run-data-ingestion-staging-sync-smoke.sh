#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${DATA_INGESTION_STAGING_SYNC_SMOKE_REPORT_DIR:-${repo_root}/target/data-ingestion-staging-sync-smoke}"
report_basename="data-ingestion-staging-sync-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
cargo_bin="${CARGO_BIN:-cargo}"
npm_bin="${NPM_BIN:-npm}"
skip_guide_check="${DATA_INGESTION_STAGING_SYNC_SMOKE_SKIP_GUIDE_CHECK:-false}"

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

if [[ "${skip_guide_check}" != "true" ]] && ! command -v "${npm_bin}" >/dev/null 2>&1; then
  echo "npm was not found. Install Node/npm, set NPM_BIN=/path/to/npm, or set DATA_INGESTION_STAGING_SYNC_SMOKE_SKIP_GUIDE_CHECK=true." >&2
  exit 1
fi

mkdir -p "${report_dir}"

head_short="$(git rev-parse --short HEAD)"
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "Data-ingestion staging sync smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Report directory: ${report_dir}"
echo "Cargo: ${cargo_bin}"
if [[ "${skip_guide_check}" == "true" ]]; then
  echo "Guide check: skipped by DATA_INGESTION_STAGING_SYNC_SMOKE_SKIP_GUIDE_CHECK=true"
else
  echo "npm: ${npm_bin}"
fi

checks=()

run_check() {
  local name="$1"
  shift
  echo ""
  echo "== ${name} =="
  "$@"
  checks+=("${name}")
}

run_check "platform-api data-ingestion analysis, staging plan, confirm, and sync contract" \
  "${cargo_bin}" test -p platform-api data_ingestion --lib
run_check "platform-api ExternalSourceSync contract" \
  "${cargo_bin}" test -p platform-api external_source_sync --lib
run_check "external-source-worker source materialization" \
  "${cargo_bin}" test -p external-source-worker
run_check "ingest-worker source document ingestion" \
  "${cargo_bin}" test -p ingest-worker
run_check "retrieval-worker source indexing" \
  "${cargo_bin}" test -p retrieval-worker

guide_check_status="skipped"
guide_check_reason="set DATA_INGESTION_STAGING_SYNC_SMOKE_SKIP_GUIDE_CHECK=false with npm available to validate generated public integration docs"
if [[ "${skip_guide_check}" != "true" ]]; then
  run_check "pure third-party guide HTML contract" \
    "${npm_bin}" run check:pure-third-party-guide-html
  guide_check_status="passed"
  guide_check_reason=""
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
SMOKE_GUIDE_CHECK_STATUS="${guide_check_status}" \
SMOKE_GUIDE_CHECK_REASON="${guide_check_reason}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const report = {
  smoke: "data-ingestion-staging-sync",
  ready: true,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  routes: {
    confirm: "POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/confirm",
    sync: "POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/sync"
  },
  third_party_statuses: [
    "data_ingestion_staging_dataset_ready",
    "data_ingestion_staging_sync_started",
    "data_ingestion_staging_sync_running",
    "data_ingestion_staging_sync_completed",
    "data_ingestion_staging_sync_failed"
  ],
  safety_contract: {
    request_fields_changed: false,
    production_write_allowed: false,
    schema_mutation_allowed: false,
    raw_database_credentials_allowed: false,
    raw_table_dump_allowed: false,
    sync_requires_confirmed_plan: true,
    sync_source_must_be_in_plan: true,
    repeat_sync_clicks_deduplicated: true
  },
  checks: JSON.parse(process.env.SMOKE_CHECKS_JSON || "[]"),
  guide_check: {
    status: process.env.SMOKE_GUIDE_CHECK_STATUS,
    reason: process.env.SMOKE_GUIDE_CHECK_REASON
  },
  notes: [
    "Default smoke is non-destructive and does not load /etc/aiv3/aiv3.env.",
    "The route tests use local test fixtures rather than live customer databases.",
    "8-server manual smoke should confirm a real stored database source can materialize into the confirmed staging dataset before customer-facing reporting."
  ]
};
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# Data Ingestion Staging Sync Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  "",
  "## Routes",
  "",
  `- Confirm: \`${report.routes.confirm}\``,
  `- Sync: \`${report.routes.sync}\``,
  "",
  "## Third-Party Statuses",
  "",
  ...report.third_party_statuses.map((status) => `- \`${status}\``),
  "",
  "## Safety Contract",
  "",
  ...Object.entries(report.safety_contract).map(([key, value]) => `- ${key}: ${value}`),
  "",
  "## Checks",
  "",
  ...report.checks.map((check) => `- ${check.status}: \`${check.name}\``),
  "",
  "## Guide Check",
  "",
  `- Status: ${report.guide_check.status}`,
  report.guide_check.reason ? `- Reason: ${report.guide_check.reason}` : "",
  "",
  "## Notes",
  "",
  ...report.notes.map((note) => `- ${note}`),
  "",
].filter((line) => line !== "").join("\n");
fs.writeFileSync(1, lines);
NODE

echo ""
echo "Data-ingestion staging sync smoke report: ${report_json}"
echo "Data-ingestion staging sync smoke summary: ${report_md}"
echo "OK data-ingestion-staging-sync smoke completed."
