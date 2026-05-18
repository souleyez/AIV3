#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${INGEST_RUNTIME_GATE_REPORT_DIR:-${repo_root}/target/ingest-runtime-gate}"
report_basename="ingest-runtime-gate-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
required_markitdown_version="${MARKITDOWN_REQUIRED_VERSION:-0.1.5}"
aiv3_env_file="${AIV3_ENV_FILE:-/etc/aiv3/aiv3.env}"

if [[ -z "${PYTHON_BIN:-}" && -r "${aiv3_env_file}" ]]; then
  python_from_env="$(awk -F= '$1 == "PYTHON_BIN" { sub(/^[^=]*=/, ""); print; exit }' "${aiv3_env_file}")"
  python_from_env="${python_from_env%\"}"
  python_from_env="${python_from_env#\"}"
  python_from_env="${python_from_env%\'}"
  python_from_env="${python_from_env#\'}"
  if [[ -n "${python_from_env}" ]]; then
    PYTHON_BIN="${python_from_env}"
  fi
fi

if [[ -z "${PYTHON_BIN:-}" && -x "/srv/aiv3/venv/media/bin/python" ]]; then
  PYTHON_BIN="/srv/aiv3/venv/media/bin/python"
fi

if [[ -z "${PYTHON_BIN:-}" ]]; then
  PYTHON_BIN="python3"
fi

mkdir -p "${report_dir}"

head_short="$(git rev-parse --short HEAD)"
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "Ingest runtime gate started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Report directory: ${report_dir}"
echo "PYTHON_BIN: ${PYTHON_BIN}"
echo "Required MarkItDown: ${required_markitdown_version}"

if [[ "${PYTHON_BIN}" == */* && ! -x "${PYTHON_BIN}" ]]; then
  echo "PYTHON_BIN is not executable: ${PYTHON_BIN}" >&2
  exit 1
fi

if ! command -v "${PYTHON_BIN}" >/dev/null 2>&1 && [[ ! -x "${PYTHON_BIN}" ]]; then
  echo "PYTHON_BIN was not found: ${PYTHON_BIN}" >&2
  exit 1
fi

markitdown_version_output="$("${PYTHON_BIN}" -m markitdown --version 2>&1)" || {
  echo "MarkItDown check failed. Expected: ${PYTHON_BIN} -m markitdown --version" >&2
  echo "${markitdown_version_output}" >&2
  exit 1
}

echo "MarkItDown: ${markitdown_version_output}"

if [[ "${markitdown_version_output}" != *"markitdown ${required_markitdown_version}"* ]]; then
  echo "Unexpected MarkItDown version. Expected markitdown ${required_markitdown_version}, got: ${markitdown_version_output}" >&2
  exit 1
fi

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

INGEST_GATE_REPO_ROOT="${repo_root}" \
INGEST_GATE_HEAD="${head_short}" \
INGEST_GATE_STARTED_AT="${started_at}" \
INGEST_GATE_FINISHED_AT="${finished_at}" \
INGEST_GATE_PYTHON_BIN="${PYTHON_BIN}" \
INGEST_GATE_MARKITDOWN_VERSION="${markitdown_version_output}" \
INGEST_GATE_REQUIRED_MARKITDOWN_VERSION="${required_markitdown_version}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const report = {
  gate: "ingest-runtime-dependencies",
  ready: true,
  repository: process.env.INGEST_GATE_REPO_ROOT,
  head: process.env.INGEST_GATE_HEAD,
  started_at: process.env.INGEST_GATE_STARTED_AT,
  finished_at: process.env.INGEST_GATE_FINISHED_AT,
  contract: {
    primary_parser_path: "V3 local ingest parsers remain the primary path.",
    fallback_parser: "MarkItDown is enabled only as the generic fallback parser.",
    deployment_guard: "Deployment target must pass PYTHON_BIN -m markitdown --version with the pinned version before ingest fallback is considered ready."
  },
  python_bin: process.env.INGEST_GATE_PYTHON_BIN,
  required_markitdown_version: process.env.INGEST_GATE_REQUIRED_MARKITDOWN_VERSION,
  observed_markitdown_version: process.env.INGEST_GATE_MARKITDOWN_VERSION,
  checks: [
    { name: "PYTHON_BIN is executable or resolvable", status: "passed" },
    { name: "PYTHON_BIN -m markitdown --version exits successfully", status: "passed" },
    { name: "MarkItDown version matches pinned fallback version", status: "passed" }
  ],
  notes: [
    "This gate is non-destructive and does not parse customer documents.",
    "Run on deployment targets after environment setup and before relying on MarkItDown fallback parsing.",
    "Future A/B parsing quality comparisons for DOCX/PDF/PPTX should run as a separate evidence smoke."
  ]
};
fs.writeFileSync(1, JSON.stringify(report, null, 2));
NODE

INGEST_GATE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.INGEST_GATE_REPORT_JSON, "utf8"));
const lines = [
  "# Ingest Runtime Dependency Gate",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  `- PYTHON_BIN: ${report.python_bin}`,
  `- Required MarkItDown: ${report.required_markitdown_version}`,
  `- Observed MarkItDown: ${report.observed_markitdown_version}`,
  "",
  "## Contract",
  "",
  `- Primary parser path: ${report.contract.primary_parser_path}`,
  `- Fallback parser: ${report.contract.fallback_parser}`,
  `- Deployment guard: ${report.contract.deployment_guard}`,
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
echo "Ingest runtime gate report: ${report_json}"
echo "Ingest runtime gate summary: ${report_md}"
echo "OK ingest-runtime dependency gate completed."
