#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${CUSTOMER_WEB_CODEX_SMOKE_REPORT_DIR:-${repo_root}/target/customer-web-codex-executor-smoke}"
report_basename="customer-web-codex-executor-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
live_self_test_output_dir="${report_dir}/${report_basename}-live-self-test"
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
  env CUSTOMER_WEB_CODEX_LIVE_SMOKE_OUTPUT_DIR="${live_self_test_output_dir}" \
    npm run smoke:customer-web-codex-live -- --self-test

echo ""
echo "== customer-web-codex-live self-test evidence readback =="
live_self_test_evidence_json="$(
  CUSTOMER_WEB_CODEX_LIVE_SELF_TEST_OUTPUT_DIR="${live_self_test_output_dir}" node <<'NODE'
const fs = require("fs");
const path = require("path");

const dir = process.env.CUSTOMER_WEB_CODEX_LIVE_SELF_TEST_OUTPUT_DIR;
const files = fs.readdirSync(dir)
  .filter((name) => name.endsWith(".json"))
  .map((name) => {
    const filePath = path.join(dir, name);
    return { name, filePath, mtimeMs: fs.statSync(filePath).mtimeMs };
  })
  .sort((left, right) => right.mtimeMs - left.mtimeMs);
if (!files.length) {
  throw new Error("customer web codex live self-test report was not written");
}
const selected = files[0];
const report = JSON.parse(fs.readFileSync(selected.filePath, "utf8"));
const checkNames = new Set(
  (Array.isArray(report.checks) ? report.checks : [])
    .filter((check) => check && check.status === "passed")
    .map((check) => String(check.name || "")),
);
const synthetic = report.syntheticShelfEvidence || {};
const syntheticShelfEvidenceSummary = {
  case_count: Number(synthetic.caseCount || 0),
  task_card_case_count: Number(synthetic.taskCardCaseCount || 0),
  artifact_bundle_case_count: Number(synthetic.artifactBundleCaseCount || 0),
  blocked_task_case_count: Number(synthetic.blockedTaskCaseCount || 0),
  product_change_artifact_bundle_count: Number(synthetic.productChangeArtifactBundleCount || 0),
  all_synthetic_cases_met: synthetic.allSyntheticCasesMet === true,
};
const evidence = {
  report_basename: selected.name,
  ok: report.ok === true,
  mode_self_test: report.mode === "self-test",
  live_writes_attempted: report.liveWritesAttempted === true,
  live_writes_blocked: report.liveWritesAttempted === false,
  approval_gate_enforced: checkNames.has("approval_gate_requires_ack_approval_auth_dataset_and_artifact"),
  current_artifact_shape_gate_enforced: checkNames.has(
    "current_static_page_artifact_shape_rejects_placeholder_context",
  ),
  current_artifact_public_url_shorthand_ready: checkNames.has(
    "current_static_page_artifact_public_url_shorthand_builds_valid_context",
  ),
  sse_parser_ready: checkNames.has("sse_parser_extracts_completed_response"),
  synthetic_five_case_evidence_matrix_ready: checkNames.has("synthetic_five_case_evidence_matrix"),
  product_change_blocked_no_artifact_ready: checkNames.has(
    "blocked_product_change_evidence_has_no_artifact_bundle",
  ),
  report_redaction_ready: checkNames.has("report_redaction_rejects_auth_and_prompt_secrets"),
  synthetic_shelf_evidence_ready:
    syntheticShelfEvidenceSummary.case_count === 5
    && syntheticShelfEvidenceSummary.task_card_case_count === 5
    && syntheticShelfEvidenceSummary.artifact_bundle_case_count === 3
    && syntheticShelfEvidenceSummary.blocked_task_case_count === 1
    && syntheticShelfEvidenceSummary.product_change_artifact_bundle_count === 0
    && syntheticShelfEvidenceSummary.all_synthetic_cases_met,
  synthetic_shelf_evidence_summary: syntheticShelfEvidenceSummary,
};
evidence.ready = evidence.ok
  && evidence.mode_self_test
  && evidence.live_writes_blocked
  && evidence.approval_gate_enforced
  && evidence.current_artifact_shape_gate_enforced
  && evidence.current_artifact_public_url_shorthand_ready
  && evidence.sse_parser_ready
  && evidence.synthetic_five_case_evidence_matrix_ready
  && evidence.product_change_blocked_no_artifact_ready
  && evidence.report_redaction_ready
  && evidence.synthetic_shelf_evidence_ready;
if (!evidence.ready) {
  throw new Error(`customer web codex live self-test evidence is incomplete: ${JSON.stringify(evidence)}`);
}
process.stdout.write(JSON.stringify(evidence));
NODE
)"
checks+=("customer-web-codex-live self-test evidence readback")
echo "${live_self_test_evidence_json}" | node -e 'const fs = require("fs"); const evidence = JSON.parse(fs.readFileSync(0, "utf8")); console.log(`evidence_ready=${evidence.ready} synthetic_cases=${evidence.synthetic_shelf_evidence_summary.case_count} artifact_bundles=${evidence.synthetic_shelf_evidence_summary.artifact_bundle_case_count}`);'

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
SMOKE_LIVE_SELF_TEST_EVIDENCE_JSON="${live_self_test_evidence_json}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const liveSelfTestEvidence = JSON.parse(process.env.SMOKE_LIVE_SELF_TEST_EVIDENCE_JSON || "{}");
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
  acceptance_status: {
    schema: "v3.customer_web_codex_executor_acceptance_status.v1",
    full_acceptance_ready: false,
    no_live_gate: {
      status: "passed",
      check_count: JSON.parse(process.env.SMOKE_CHECKS_JSON || "[]").length,
      video_ppt_no_live_rollup_status: process.env.SMOKE_VIDEO_PPT_ROLLUP_STATUS,
      proves_controlled_live: false,
      live_self_test_evidence_ready: liveSelfTestEvidence.ready === true,
      live_self_test_report: liveSelfTestEvidence.report_basename || ""
    },
    live_gate_readiness_summary: {
      schema: "v3.customer_web_codex_executor_live_gate_readiness_summary.v1",
      controlled_live_harness_self_test_ready: liveSelfTestEvidence.ready === true,
      approval_gate_enforced: liveSelfTestEvidence.approval_gate_enforced === true,
      current_artifact_shape_gate_enforced:
        liveSelfTestEvidence.current_artifact_shape_gate_enforced === true,
      current_artifact_public_url_shorthand_ready:
        liveSelfTestEvidence.current_artifact_public_url_shorthand_ready === true,
      sse_parser_ready: liveSelfTestEvidence.sse_parser_ready === true,
      right_side_shelf_synthetic_evidence_ready:
        liveSelfTestEvidence.synthetic_shelf_evidence_ready === true,
      product_change_blocked_no_artifact_ready:
        liveSelfTestEvidence.product_change_blocked_no_artifact_ready === true,
      report_redaction_ready: liveSelfTestEvidence.report_redaction_ready === true,
      synthetic_shelf_evidence_summary:
        liveSelfTestEvidence.synthetic_shelf_evidence_summary || null,
      required_input_count: 4,
      required_inputs: [
        "test account session cookie or bearer",
        "controlled test dataset id",
        "current rendered V3 generated-artifact URL or artifact context",
        "operator approval id/reference"
      ],
      live_smoke_pending: true
    },
    pending_gate_requirements_summary: {
      schema: "v3.customer_web_codex_executor_pending_gate_requirements_summary.v1",
      pending_gate_count: 6,
      pending_gates: [
        "live Right Code customer_complex_request smoke",
        "live Right Code customer_artifact_request smoke",
        "live generated_static_page_edit smoke",
        "live generated_static_page_publish smoke",
        "live v3_product_change_request operator-review smoke",
        "live right-side shelf task/artifact visibility smoke"
      ],
      no_live_substitute_available_for_pending_gates: true
    },
    safety_summary: {
      no_provider_secrets_read: true,
      no_live_api_calls: true,
      live_self_test_writes_blocked: liveSelfTestEvidence.live_writes_blocked === true,
      no_deploy_or_service_restart: true,
      no_120_touched: true,
      screen_recording_enabled: false
    }
  },
  external_gates_not_executed_by_this_smoke: [
    "GitHub push or deployment approval",
    "deployment-target service restart or rollout",
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
    "This smoke does not execute controlled live Customer Web Codex writes.",
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
  "## Acceptance Status",
  "",
  `- Schema: ${report.acceptance_status.schema}`,
  `- Full acceptance ready: ${report.acceptance_status.full_acceptance_ready}`,
  `- No-live gate: ${report.acceptance_status.no_live_gate.status}`,
  `- No-live check count: ${report.acceptance_status.no_live_gate.check_count}`,
  `- Video/PPT no-live rollup: ${report.acceptance_status.no_live_gate.video_ppt_no_live_rollup_status}`,
  `- Live self-test evidence ready: ${report.acceptance_status.no_live_gate.live_self_test_evidence_ready}`,
  `- Synthetic shelf cases: ${report.acceptance_status.live_gate_readiness_summary.synthetic_shelf_evidence_summary?.case_count ?? "unknown"}`,
  `- Synthetic shelf artifact-bundle cases: ${report.acceptance_status.live_gate_readiness_summary.synthetic_shelf_evidence_summary?.artifact_bundle_case_count ?? "unknown"}`,
  `- Controlled live smoke pending: ${report.acceptance_status.live_gate_readiness_summary.live_smoke_pending}`,
  `- Required controlled-live inputs: ${report.acceptance_status.live_gate_readiness_summary.required_input_count}`,
  `- Pending live gate count: ${report.acceptance_status.pending_gate_requirements_summary.pending_gate_count}`,
  `- No-live substitute for pending gates: ${report.acceptance_status.pending_gate_requirements_summary.no_live_substitute_available_for_pending_gates}`,
  "",
  "## External Gates Not Executed By This Smoke",
  "",
  ...report.external_gates_not_executed_by_this_smoke.map((gate) => `- ${gate}`),
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
