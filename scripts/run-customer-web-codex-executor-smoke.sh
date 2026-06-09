#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${CUSTOMER_WEB_CODEX_SMOKE_REPORT_DIR:-${repo_root}/target/customer-web-codex-executor-smoke}"
report_basename="customer-web-codex-executor-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
include_video_ppt_rollup="${CUSTOMER_WEB_CODEX_SMOKE_INCLUDE_VIDEO_PPT_ROLLUP:-false}"
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

echo "Customer Web Codex executor smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Report directory: ${report_dir}"
echo "Include video PPT no-live rollup: ${include_video_ppt_rollup}"
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

run_check "cargo fmt --check" \
  "${cargo_bin}" fmt --check
run_check "cargo test -p platform-api customer_codex --lib" \
  "${cargo_bin}" test -p platform-api customer_codex --lib
run_check "cargo test -p platform-api codex_host --lib" \
  "${cargo_bin}" test -p platform-api codex_host --lib
run_check "cargo test -p platform-api external_channel_static_page_dataset_template --lib" \
  "${cargo_bin}" test -p platform-api external_channel_static_page_dataset_template --lib
run_check "cargo test -p codex-host-agent" \
  "${cargo_bin}" test -p codex-host-agent
run_check "node --test apps/web/app/lib/codex-customer-artifacts.test.mjs" \
  node --test apps/web/app/lib/codex-customer-artifacts.test.mjs
run_check "npm run smoke:customer-web-codex-readiness -- --self-test" \
  env CODEX_HOST_AGENT_PROFILE_ENV_KEY=OPENAI_API_KEY npm run smoke:customer-web-codex-readiness -- --self-test
run_check "bash scripts/run-customer-web-codex-readiness.sh --self-test --json-stdout" \
  bash -lc 'bash scripts/run-customer-web-codex-readiness.sh --self-test --json-stdout | node -e '\''const fs = require("fs"); const report = JSON.parse(fs.readFileSync(0, "utf8")); if (!report.ready || report.env_value_source !== "self_test_fixture" || report.profile_env_key_kind !== "rightcode_main") process.exit(1);'\'''
run_check "bash scripts/run-customer-web-codex-readiness.sh --json-stdout not-ready remediation fixture" \
  bash -lc 'tmp="$(mktemp)"; trap '\''rm -f "${tmp}"'\'' EXIT; cat >"${tmp}" <<'\''EOF'\''
ASSISTANT_RUN_RUNTIME_PROVIDER=rightcode
CODEX_HOST_AGENT_PROFILE_PROVIDER_ID=rightcode
CODEX_HOST_AGENT_PROFILE_ID=rightcode-gpt-5-5-high
CODEX_HOST_AGENT_PROFILE_ENV_KEY=OPENAI_API_KEY
CODEX_HOST_TASK_ALLOWLIST=static_page_image2_data_publish
CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES=static_page_image2_data_publish
CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=true
CODEX_HOST_AGENT_HOST_KIND=aiv3_server
CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT=/srv/aiv3/codex-workspaces
OPENAI_API_KEY=fixture-secret-not-for-output
EOF
  bash scripts/run-customer-web-codex-readiness.sh --env-file "${tmp}" --json-stdout --allow-not-ready | node -e '\''const fs = require("fs"); const text = fs.readFileSync(0, "utf8"); if (text.includes("fixture-secret-not-for-output")) process.exit(1); const report = JSON.parse(text); if (report.ready) process.exit(1); if (report.missing_task_allowlist_capabilities.length !== 4) process.exit(1); if (report.missing_profile_allowed_capabilities.length !== 4) process.exit(1); if (report.remediation.raw_secret_values_included !== false) process.exit(1); if (!report.remediation.required_env_updates.some((update) => update.key === "RIGHTCODE_API_KEY_MAIN" && update.secret_value === true)) process.exit(1);'\'''
run_check "npm run smoke:customer-web-codex-live -- --self-test" \
  npm run smoke:customer-web-codex-live -- --self-test
run_check "npm --prefix apps/web run build" \
  npm --prefix apps/web run build
run_check "git diff --check" \
  git diff --check

video_ppt_rollup_status="skipped"
video_ppt_rollup_reason="set CUSTOMER_WEB_CODEX_SMOKE_INCLUDE_VIDEO_PPT_ROLLUP=true to run the consolidated no-live video/PPT regression gate"
if [[ "${include_video_ppt_rollup}" == "true" ]]; then
  run_check "npm run smoke:video-ppt-no-live-rollup -- --self-test" \
    npm run smoke:video-ppt-no-live-rollup -- --self-test
  video_ppt_rollup_status="passed"
  video_ppt_rollup_reason=""
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
SMOKE_VIDEO_PPT_ROLLUP_STATUS="${video_ppt_rollup_status}" \
SMOKE_VIDEO_PPT_ROLLUP_REASON="${video_ppt_rollup_reason}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const report = {
  smoke: "customer-web-codex-executor",
  ready: true,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  local_scope: {
    customer_complex_request: "read-only task and safe result-summary contract",
    customer_artifact_request: "isolated task workspace and manifest/public-url safety",
    generated_static_page_edit: "seeded workspace, changed-page validation, generated-artifact handoff",
    generated_static_page_publish: "new customer page package route and artifact-ready projection",
    v3_product_change_request: "operator-review blocked outcome; no customer write task",
    right_side_shelf: "task cards plus artifact bundles with safe browser-visible fields",
    readiness: "env/profile readiness self-test uses fixture/env-file values instead of conflicting shell env"
  },
  checks: JSON.parse(process.env.SMOKE_CHECKS_JSON || "[]"),
  video_ppt_no_live_rollup: {
    status: process.env.SMOKE_VIDEO_PPT_ROLLUP_STATUS,
    reason: process.env.SMOKE_VIDEO_PPT_ROLLUP_REASON
  },
  deployment_gates_not_executed: [
    "GitHub main contains the smoke commit",
    "8-server profile/env allowlists include customer Web Codex capabilities",
    "8-server profile env key is explicitly RIGHTCODE_API_KEY_MAIN",
    "affected services deployed only after approval",
    "live Right Code customer_complex_request smoke",
    "live Right Code customer_artifact_request smoke",
    "live generated_static_page_edit smoke",
    "live generated_static_page_publish smoke",
    "live v3_product_change_request operator-review smoke",
    "live right-side shelf task/artifact visibility smoke"
  ],
  safety_notes: [
    "This smoke does not read or print provider secrets.",
    "This smoke does not deploy services, push GitHub, touch 120, or modify production env files.",
    "Use the optional video/PPT no-live rollup before release approval to confirm the Codex executor changes did not regress video PPT handoff/upload gates."
  ]
};
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# Customer Web Codex Executor Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  "",
  "## Local Scope",
  "",
  ...Object.entries(report.local_scope).map(([key, value]) => `- ${key}: ${value}`),
  "",
  "## Checks",
  "",
  ...report.checks.map((check) => `- ${check.status}: \`${check.name}\``),
  "",
  "## Video PPT No-Live Rollup",
  "",
  `- Status: ${report.video_ppt_no_live_rollup.status}`,
  report.video_ppt_no_live_rollup.reason
    ? `- Reason: ${report.video_ppt_no_live_rollup.reason}`
    : "",
  "",
  "## Deployment Gates Not Executed",
  "",
  ...report.deployment_gates_not_executed.map((gate) => `- ${gate}`),
  "",
  "## Safety Notes",
  "",
  ...report.safety_notes.map((note) => `- ${note}`),
  "",
].filter((line) => line !== null).join("\n");
fs.writeFileSync(1, lines);
NODE

echo ""
echo "Customer Web Codex executor smoke report: ${report_json}"
echo "Customer Web Codex executor smoke summary: ${report_md}"
echo "OK customer-web-codex-executor smoke completed."
