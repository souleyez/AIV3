#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${CODEX_HOST_WORKSPACE_RETENTION_SMOKE_REPORT_DIR:-${repo_root}/target/codex-host-workspace-retention-smoke}"
report_basename="codex-host-workspace-retention-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
scan_only="${CODEX_HOST_WORKSPACE_RETENTION_SCAN_ONLY:-false}"
workspace_root="${CODEX_HOST_WORKSPACE_RETENTION_ROOT:-${report_dir}/fixtures}"

mkdir -p "${report_dir}"

head_short="$(git rev-parse --short HEAD)"
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "Codex Host workspace retention smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Report directory: ${report_dir}"
echo "Workspace root: ${workspace_root}"
echo "Scan only: ${scan_only}"

SMOKE_REPO_ROOT="${repo_root}" \
SMOKE_HEAD="${head_short}" \
SMOKE_STARTED_AT="${started_at}" \
SMOKE_SCAN_ONLY="${scan_only}" \
SMOKE_WORKSPACE_ROOT="${workspace_root}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const path = require("path");

const root = process.env.SMOKE_WORKSPACE_ROOT;
const scanOnly = process.env.SMOKE_SCAN_ONLY === "true";

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value, null, 2));
}

if (!scanOnly) {
  fs.mkdirSync(root, { recursive: true });
  const now = Date.now();
  const fixtures = [
    { name: "fresh-workspace", ageHours: 2, retentionHours: 168 },
    { name: "expired-workspace", ageHours: 240, retentionHours: 168 },
  ];
  for (const fixture of fixtures) {
    const dir = path.join(root, fixture.name);
    fs.mkdirSync(dir, { recursive: true });
    writeJson(path.join(dir, "runtime.json"), {
      assistant_run_id: `fixture-${fixture.name}`,
      capability: "static_page_image2_data_publish",
      retention_policy: {
        retention_hours: fixture.retentionHours,
        cleanup_requires_operator: true,
        backup_before_delete: true,
        normal_cleanup_mode: "operator_explicit",
        local_cleanup_helper: "Safe-RemoveToBackup.ps1",
        server_cleanup_note: "Archive workspace contents before deletion; never run cleanup from normal polling."
      },
      raw_prompt_exposed: false,
      secrets_exposed: false
    });
    const timestamp = new Date(now - fixture.ageHours * 60 * 60 * 1000);
    fs.utimesSync(dir, timestamp, timestamp);
    fs.utimesSync(path.join(dir, "runtime.json"), timestamp, timestamp);
  }
}

const entries = fs.existsSync(root)
  ? fs.readdirSync(root, { withFileTypes: true }).filter((entry) => entry.isDirectory())
  : [];
const workspaces = [];
for (const entry of entries) {
  const dir = path.join(root, entry.name);
  const runtimePath = path.join(dir, "runtime.json");
  if (!fs.existsSync(runtimePath)) {
    workspaces.push({
      name: entry.name,
      runtime_json_present: false,
      cleanup_candidate: false,
      reason: "runtime_json_missing"
    });
    continue;
  }
  let runtime;
  try {
    runtime = JSON.parse(fs.readFileSync(runtimePath, "utf8"));
  } catch (error) {
    workspaces.push({
      name: entry.name,
      runtime_json_present: true,
      cleanup_candidate: false,
      reason: "runtime_json_invalid"
    });
    continue;
  }
  const stat = fs.statSync(dir);
  const retention = runtime.retention_policy || {};
  const retentionHours = Math.max(1, Number(retention.retention_hours || 168));
  const ageHours = Math.max(0, (Date.now() - stat.mtimeMs) / 3600000);
  const cleanupRequiresOperator = retention.cleanup_requires_operator === true;
  const backupBeforeDelete = retention.backup_before_delete === true;
  const cleanupCandidate = ageHours >= retentionHours && cleanupRequiresOperator && backupBeforeDelete;
  workspaces.push({
    name: entry.name,
    runtime_json_present: true,
    retention_hours: retentionHours,
    age_hours: Number(ageHours.toFixed(2)),
    cleanup_requires_operator: cleanupRequiresOperator,
    backup_before_delete: backupBeforeDelete,
    cleanup_candidate: cleanupCandidate,
    reason: cleanupCandidate ? "retention_window_elapsed" : "within_retention_or_policy_incomplete"
  });
}

const finishedAt = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
const report = {
  smoke: "codex-host-workspace-retention",
  ready: true,
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: finishedAt,
  workspace_root: root,
  scan_only: scanOnly,
  destructive_operation_performed: false,
  safety_contract: {
    normal_polling_deletes_workspaces: false,
    cleanup_requires_operator: true,
    backup_before_delete: true,
    local_cleanup_helper: "Safe-RemoveToBackup.ps1",
    server_cleanup_mode: "operator_explicit_archive_then_delete"
  },
  workspace_count: workspaces.length,
  cleanup_candidate_count: workspaces.filter((workspace) => workspace.cleanup_candidate).length,
  workspaces
};
process.stdout.write(JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# Codex Host Workspace Retention Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  `- Workspace root: ${report.workspace_root}`,
  `- Scan only: ${report.scan_only}`,
  `- Destructive operation performed: ${report.destructive_operation_performed}`,
  `- Workspace count: ${report.workspace_count}`,
  `- Cleanup candidate count: ${report.cleanup_candidate_count}`,
  "",
  "## Safety Contract",
  "",
  ...Object.entries(report.safety_contract).map(([key, value]) => `- ${key}: ${value}`),
  "",
  "## Workspaces",
  "",
  ...report.workspaces.map((workspace) => {
    if (!workspace.runtime_json_present) {
      return `- ${workspace.name}: skipped (${workspace.reason})`;
    }
    return `- ${workspace.name}: candidate=${workspace.cleanup_candidate}, age_hours=${workspace.age_hours}, retention_hours=${workspace.retention_hours}, reason=${workspace.reason}`;
  }),
  "",
].join("\n");
fs.writeFileSync(1, lines);
NODE

echo ""
echo "Codex Host workspace retention smoke report: ${report_json}"
echo "Codex Host workspace retention smoke summary: ${report_md}"
echo "OK codex-host-workspace-retention smoke completed."
