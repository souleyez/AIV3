#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${CUSTOMER_WEB_CODEX_REPO_ROOT:-}" ]]; then
  repo_root="${CUSTOMER_WEB_CODEX_REPO_ROOT}"
else
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "${script_dir}/.." && pwd)"
fi
cd "${repo_root}"

report_dir="${CUSTOMER_WEB_CODEX_READINESS_REPORT_DIR:-${repo_root}/target/customer-web-codex-readiness}"
report_basename="customer-web-codex-readiness-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
env_file="${CUSTOMER_WEB_CODEX_ENV_FILE:-/etc/aiv3/aiv3.env}"
self_test="${CUSTOMER_WEB_CODEX_READINESS_SELF_TEST:-false}"
allow_not_ready="${CUSTOMER_WEB_CODEX_READINESS_ALLOW_NOT_READY:-false}"
json_stdout="${CUSTOMER_WEB_CODEX_READINESS_JSON_STDOUT:-false}"

print_usage() {
  cat <<'EOF'
Usage:
  bash scripts/run-customer-web-codex-readiness.sh [--self-test] [--env-file PATH] [--allow-not-ready] [--json-stdout]

Options:
  --self-test        Use synthetic safe env fixtures.
  --env-file PATH    Parse a deployment env file without sourcing it.
  --allow-not-ready  Write the report but do not fail the command when readiness is false.
  --json-stdout      Print redacted JSON to stdout and do not write report files.
  --help             Show this help text.

The script prints only booleans, key names, and non-secret config labels.
It never prints provider key values.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --self-test)
      self_test="true"
      ;;
    --env-file)
      env_file="${2:-}"
      if [[ -z "${env_file}" ]]; then
        echo "--env-file requires a path" >&2
        exit 1
      fi
      shift
      ;;
    --allow-not-ready)
      allow_not_ready="true"
      ;;
    --json-stdout)
      json_stdout="true"
      ;;
    --help|-h)
      print_usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      print_usage >&2
      exit 1
      ;;
  esac
  shift
done

if [[ "${json_stdout}" != "true" ]]; then
  mkdir -p "${report_dir}"
fi

head_short="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

if [[ "${json_stdout}" != "true" ]]; then
  echo "Customer Web Codex readiness check started"
  echo "Repository: ${repo_root}"
  echo "HEAD: ${head_short}"
  echo "Report directory: ${report_dir}"
  echo "Env file: ${env_file}"
  echo "Self-test: ${self_test}"
fi

report_json_payload="$(
  SMOKE_REPO_ROOT="${repo_root}" \
  SMOKE_HEAD="${head_short}" \
  SMOKE_STARTED_AT="${started_at}" \
  SMOKE_ENV_FILE="${env_file}" \
  SMOKE_SELF_TEST="${self_test}" \
  node <<'NODE'
const fs = require("fs");
const path = require("path");

const requiredCapabilities = [
  "customer_complex_request",
  "customer_artifact_request",
  "generated_static_page_edit",
  "generated_static_page_publish",
];

function parseEnvLineValue(raw) {
  let value = String(raw || "").trim();
  if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
    value = value.slice(1, -1);
  }
  return value;
}

function parseEnvText(text) {
  const parsed = {};
  for (const rawLine of String(text || "").split(/\r?\n/)) {
    let line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    if (line.startsWith("export ")) {
      line = line.slice("export ".length).trim();
    }
    const match = line.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/);
    if (!match) continue;
    parsed[match[1]] = parseEnvLineValue(match[2]);
  }
  return parsed;
}

function csvSet(value) {
  return new Set(
    String(value || "")
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean),
  );
}

function missingCapabilities(value) {
  const values = csvSet(value);
  return requiredCapabilities.filter((capability) => !values.has(capability));
}

function hasAllCapabilities(value) {
  return missingCapabilities(value).length === 0;
}

function profileEnvKeyKind(value) {
  if (value === "RIGHTCODE_API_KEY_MAIN") return "rightcode_main";
  if (value === "OPENAI_API_KEY") return "openai_api_key_alias";
  if (!value) return "missing";
  return "other";
}

function isTruthy(value) {
  const normalized = String(value || "").trim().toLowerCase();
  return ["1", "true", "yes", "on"].includes(normalized);
}

function normalizePath(value) {
  const raw = String(value || "").trim();
  return raw.replace(/\/+$/, "") || raw;
}

function pathIsRepoOrChild(candidate, repoPath) {
  const normalizedCandidate = normalizePath(candidate);
  const normalizedRepo = normalizePath(repoPath);
  return normalizedCandidate === normalizedRepo
    || normalizedCandidate.startsWith(`${normalizedRepo}/`);
}

const envFile = process.env.SMOKE_ENV_FILE || "/etc/aiv3/aiv3.env";
const selfTest = process.env.SMOKE_SELF_TEST === "true";
const syntheticEnv = {
  ASSISTANT_RUN_RUNTIME_PROVIDER: "rightcode",
  CODEX_HOST_TASK_ALLOWLIST: "inspect_project,ops_readonly,diagnose,explain,customer_complex_request,customer_artifact_request,generated_static_page_edit,generated_static_page_publish,static_page_image2_data_publish,data_ingestion_analysis",
  CODEX_HOST_AGENT_PROFILE_ID: "rightcode-gpt-5-5-high",
  CODEX_HOST_AGENT_PROFILE_PROVIDER_ID: "rightcode",
  CODEX_HOST_AGENT_PROFILE_ENV_KEY: "RIGHTCODE_API_KEY_MAIN",
  CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES: "customer_complex_request,customer_artifact_request,generated_static_page_edit,generated_static_page_publish,static_page_image2_data_publish,data_ingestion_analysis",
  RIGHTCODE_API_KEY_MAIN: "synthetic-secret-not-written",
  CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC: "true",
  CODEX_HOST_AGENT_HOST_KIND: "aiv3_server",
  CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT: "/srv/aiv3/codex-workspaces",
};

let envFileStatus = "not_read";
let envFileValues = {};
if (selfTest) {
  envFileStatus = "self_test_fixture";
  envFileValues = syntheticEnv;
} else if (fs.existsSync(envFile)) {
  envFileStatus = "read";
  envFileValues = parseEnvText(fs.readFileSync(envFile, "utf8"));
} else {
  envFileStatus = "missing";
}

const envUsesFixtureOrFile = selfTest || envFileStatus === "read";
const env = envUsesFixtureOrFile ? envFileValues : process.env;
const envValueSource = selfTest
  ? "self_test_fixture"
  : envFileStatus === "read"
    ? "env_file"
    : "process_env_fallback";
const provider = env.CODEX_HOST_AGENT_PROFILE_PROVIDER_ID || env.ASSISTANT_RUN_RUNTIME_PROVIDER || "";
const profile = env.CODEX_HOST_AGENT_PROFILE_ID || "";
const envKey = env.CODEX_HOST_AGENT_PROFILE_ENV_KEY || "";
const workspaceRoot = env.CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT || "";
const hostKind = env.CODEX_HOST_AGENT_HOST_KIND || "";
const approvedHostKinds = new Set(["windows_jump", "mac_host", "linux_host", "aiv3_server", "cloudflare_codex"]);
const profileEnvKind = profileEnvKeyKind(envKey);
const taskAllowlistOk = hasAllCapabilities(env.CODEX_HOST_TASK_ALLOWLIST);
const profileAllowlistOk = hasAllCapabilities(env.CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES);
const missingTaskAllowlistCapabilities = missingCapabilities(env.CODEX_HOST_TASK_ALLOWLIST);
const missingProfileAllowedCapabilities = missingCapabilities(env.CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES);
const workspaceRootConfigured = Boolean(workspaceRoot);
const taskWorkspaceRootNotRepo = workspaceRootConfigured
  && !pathIsRepoOrChild(workspaceRoot, "/srv/aiv3/repo")
  && !pathIsRepoOrChild(workspaceRoot, process.env.SMOKE_REPO_ROOT || "");
const checks = {
  provider_is_rightcode: provider === "rightcode",
  profile_is_rightcode_high: profile === "rightcode-gpt-5-5-high",
  profile_env_key_kind: profileEnvKind,
  profile_env_key_is_rightcode_main: profileEnvKind === "rightcode_main",
  rightcode_named_key_ready: Boolean(env.RIGHTCODE_API_KEY_MAIN),
  task_workspace_root_configured: workspaceRootConfigured,
  task_workspace_root_not_repo: taskWorkspaceRootNotRepo,
  task_allowlist_ok: taskAllowlistOk,
  profile_allowed_capabilities_ok: profileAllowlistOk,
  allow_real_codex_exec: isTruthy(env.CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC),
  host_kind_approved: approvedHostKinds.has(hostKind),
};
const ready = checks.provider_is_rightcode
  && checks.profile_is_rightcode_high
  && checks.profile_env_key_is_rightcode_main
  && checks.rightcode_named_key_ready
  && checks.task_workspace_root_configured
  && checks.task_workspace_root_not_repo
  && checks.task_allowlist_ok
  && checks.profile_allowed_capabilities_ok
  && checks.allow_real_codex_exec
  && checks.host_kind_approved;
const remediation = {
  production_write_approval_required: true,
  raw_secret_values_included: false,
  required_env_updates: [
    ...(checks.profile_env_key_is_rightcode_main ? [] : [{
      key: "CODEX_HOST_AGENT_PROFILE_ENV_KEY",
      action: "set_literal",
      value: "RIGHTCODE_API_KEY_MAIN",
      secret_value: false,
    }]),
    ...(checks.rightcode_named_key_ready ? [] : [{
      key: "RIGHTCODE_API_KEY_MAIN",
      action: "set_secret_from_approved_source",
      value: "[REDACTED_REQUIRED]",
      secret_value: true,
    }]),
    ...(missingTaskAllowlistCapabilities.length === 0 ? [] : [{
      key: "CODEX_HOST_TASK_ALLOWLIST",
      action: "append_missing_csv_values",
      missing_values: missingTaskAllowlistCapabilities,
      secret_value: false,
    }]),
    ...(missingProfileAllowedCapabilities.length === 0 ? [] : [{
      key: "CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES",
      action: "append_missing_csv_values",
      missing_values: missingProfileAllowedCapabilities,
      secret_value: false,
    }]),
  ],
  next_checks: [
    "rerun readiness with --json-stdout and require ready=true",
    "deploy only after GitHub main contains the approved commit and a deployment window is approved",
    "run live customer Web Codex smoke only after readiness is ready and affected services are deployed",
  ],
};

const finishedAt = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
const report = {
  schema: "v3.customer_web_codex_readiness.v1",
  ready,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: finishedAt,
  self_test: selfTest,
  env_file_status: envFileStatus,
  env_value_source: envValueSource,
  env_file_path_checked: envFile,
  provider,
  profile,
  profile_env_key_kind: profileEnvKind,
  host_kind: hostKind || null,
  task_workspace_root_present: workspaceRootConfigured,
  task_workspace_root_label: workspaceRootConfigured ? "configured_redacted_path" : "missing",
  required_capabilities: requiredCapabilities,
  missing_task_allowlist_capabilities: missingTaskAllowlistCapabilities,
  missing_profile_allowed_capabilities: missingProfileAllowedCapabilities,
  checks,
  remediation,
  redaction: {
    provider_key_values_printed: false,
    rightcode_value_printed: false,
    workspace_root_value_printed: false,
    env_file_values_printed: false,
  },
};
process.stdout.write(JSON.stringify(report, null, 2));
NODE
)"

if [[ "${json_stdout}" == "true" ]]; then
  printf '%s\n' "${report_json_payload}"
else
  printf '%s\n' "${report_json_payload}" >"${report_json}"
fi

if [[ "${json_stdout}" != "true" ]]; then
  SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# Customer Web Codex Readiness",
  "",
  `- Status: ${report.ready ? "ready" : "not ready"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  `- Self-test: ${report.self_test}`,
  `- Env file status: ${report.env_file_status}`,
  `- Env value source: ${report.env_value_source}`,
  `- Provider: ${report.provider || "missing"}`,
  `- Profile: ${report.profile || "missing"}`,
  `- Profile env key kind: ${report.profile_env_key_kind}`,
  `- Host kind: ${report.host_kind || "missing"}`,
  `- Task workspace root: ${report.task_workspace_root_label}`,
  "",
  "## Required Capabilities",
  "",
  ...report.required_capabilities.map((capability) => `- ${capability}`),
  "",
  "## Checks",
  "",
  ...Object.entries(report.checks).map(([key, value]) => `- ${key}: ${value}`),
  "",
  "## Missing Capabilities",
  "",
  `- CODEX_HOST_TASK_ALLOWLIST: ${report.missing_task_allowlist_capabilities.length ? report.missing_task_allowlist_capabilities.join(", ") : "none"}`,
  `- CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES: ${report.missing_profile_allowed_capabilities.length ? report.missing_profile_allowed_capabilities.join(", ") : "none"}`,
  "",
  "## Remediation",
  "",
  `- Production write approval required: ${report.remediation.production_write_approval_required}`,
  `- Raw secret values included: ${report.remediation.raw_secret_values_included}`,
  ...report.remediation.required_env_updates.map((update) => {
    const value = update.missing_values
      ? update.missing_values.join(", ")
      : update.value || "";
    return `- ${update.key}: ${update.action}${value ? ` (${value})` : ""}`;
  }),
  "",
  "## Redaction",
  "",
  ...Object.entries(report.redaction).map(([key, value]) => `- ${key}: ${value}`),
  "",
].join("\n");
fs.writeFileSync(1, lines);
NODE
fi

ready_status="$(
  printf '%s' "${report_json_payload}" \
    | node -e 'const fs=require("fs"); const r=JSON.parse(fs.readFileSync(0,"utf8")); process.stdout.write(r.ready ? "ready" : "not_ready");'
)"

if [[ "${json_stdout}" != "true" ]]; then
  echo ""
  echo "Customer Web Codex readiness report: ${report_json}"
  echo "Customer Web Codex readiness summary: ${report_md}"
  echo "Readiness: ${ready_status}"
fi

if [[ "${ready_status}" != "ready" && "${allow_not_ready}" != "true" ]]; then
  echo "Customer Web Codex readiness check failed. Re-run with --allow-not-ready only when collecting diagnostics." >&2
  exit 1
fi

if [[ "${json_stdout}" != "true" ]]; then
  echo "OK customer-web-codex-readiness completed."
fi
