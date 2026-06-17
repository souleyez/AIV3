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
required_paddleocr_version="${PADDLEOCR_REQUIRED_VERSION:-3.7.0}"
aiv3_env_file="${AIV3_ENV_FILE:-/etc/aiv3/aiv3.env}"

read_aiv3_env_value() {
  local key="$1"
  awk -F= -v key="${key}" '$1 == key { sub(/^[^=]*=/, ""); print; exit }' "${aiv3_env_file}" \
    | sed -e 's/^"//' -e 's/"$//' -e "s/^'//" -e "s/'$//"
}

env_truthy() {
  case "$(printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]' | xargs)" in
    1 | true | yes | on) return 0 ;;
    *) return 1 ;;
  esac
}

if [[ -z "${PYTHON_BIN:-}" && -r "${aiv3_env_file}" ]]; then
  python_from_env="$(read_aiv3_env_value "PYTHON_BIN")"
  if [[ -n "${python_from_env}" ]]; then
    PYTHON_BIN="${python_from_env}"
  fi
fi

if [[ -z "${DOCUMENT_PADDLEOCR_ENABLED:-}" && -r "${aiv3_env_file}" ]]; then
  DOCUMENT_PADDLEOCR_ENABLED="$(read_aiv3_env_value "DOCUMENT_PADDLEOCR_ENABLED")"
fi

if [[ -z "${DOCUMENT_PDF_PARSE_ENGINE:-}" && -r "${aiv3_env_file}" ]]; then
  DOCUMENT_PDF_PARSE_ENGINE="$(read_aiv3_env_value "DOCUMENT_PDF_PARSE_ENGINE")"
fi

if [[ -z "${DOCUMENT_PADDLEOCR_PYTHON_BIN:-}" && -r "${aiv3_env_file}" ]]; then
  DOCUMENT_PADDLEOCR_PYTHON_BIN="$(read_aiv3_env_value "DOCUMENT_PADDLEOCR_PYTHON_BIN")"
fi

if [[ -z "${DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN:-}" && -r "${aiv3_env_file}" ]]; then
  DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN="$(read_aiv3_env_value "DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN")"
fi

if [[ -z "${PYTHON_BIN:-}" && -x "/srv/aiv3/venv/media/bin/python" ]]; then
  PYTHON_BIN="/srv/aiv3/venv/media/bin/python"
fi

if [[ -z "${PYTHON_BIN:-}" ]]; then
  PYTHON_BIN="python3"
fi

if [[ -z "${DOCUMENT_PADDLEOCR_PYTHON_BIN:-}" && -x "/srv/aiv3/venv/paddleocr/bin/python" ]]; then
  DOCUMENT_PADDLEOCR_PYTHON_BIN="/srv/aiv3/venv/paddleocr/bin/python"
fi

if [[ -z "${DOCUMENT_PADDLEOCR_PYTHON_BIN:-}" && -n "${DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN:-}" ]]; then
  DOCUMENT_PADDLEOCR_PYTHON_BIN="${DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN}"
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
echo "Required PaddleOCR: ${required_paddleocr_version}"
echo "DOCUMENT_PADDLEOCR_ENABLED: ${DOCUMENT_PADDLEOCR_ENABLED:-}"
echo "DOCUMENT_PDF_PARSE_ENGINE: ${DOCUMENT_PDF_PARSE_ENGINE:-}"
echo "DOCUMENT_PADDLEOCR_PYTHON_BIN: ${DOCUMENT_PADDLEOCR_PYTHON_BIN:-}"
echo "DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN: ${DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN:-}"

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

paddleocr_required="false"
if env_truthy "${INGEST_GATE_REQUIRE_PADDLEOCR:-}"; then
  paddleocr_required="true"
elif [[ "${DOCUMENT_PADDLEOCR_ENABLED:-}" =~ ^([Ff][Aa][Ll][Ss][Ee]|0|[Nn][Oo]|[Oo][Ff][Ff]|[Dd][Ii][Ss][Aa][Bb][Ll][Ee][Dd])$ ]] || [[ "${DOCUMENT_PDF_PARSE_ENGINE:-}" == "native_first" ]]; then
  paddleocr_required="false"
elif env_truthy "${DOCUMENT_PADDLEOCR_ENABLED:-}" || [[ "${DOCUMENT_PDF_PARSE_ENGINE:-}" == "paddleocr_first" ]] || [[ -n "${DOCUMENT_PADDLEOCR_PYTHON_BIN:-}" ]]; then
  paddleocr_required="true"
fi

paddleocr_status="skipped"
paddleocr_smoke_status="skipped"
paddleocr_output=""
paddleocr_smoke_output=""
paddleocr_python_bin="${DOCUMENT_PADDLEOCR_PYTHON_BIN:-${PYTHON_BIN}}"
if [[ "${paddleocr_required}" == "true" ]]; then
  if [[ "${paddleocr_python_bin}" == */* && ! -x "${paddleocr_python_bin}" ]]; then
    echo "DOCUMENT_PADDLEOCR_PYTHON_BIN is not executable: ${paddleocr_python_bin}" >&2
    exit 1
  fi
  if ! command -v "${paddleocr_python_bin}" >/dev/null 2>&1 && [[ ! -x "${paddleocr_python_bin}" ]]; then
    echo "PaddleOCR Python was not found: ${paddleocr_python_bin}" >&2
    exit 1
  fi
  paddleocr_output="$("${paddleocr_python_bin}" - "${required_paddleocr_version}" <<'PY' 2>&1
import importlib.metadata as md
import sys
from paddleocr import PPStructureV3

def version_tuple(value):
    parts = []
    for part in str(value).split("."):
        digits = ""
        for char in part:
            if char.isdigit():
                digits += char
            else:
                break
        parts.append(int(digits or "0"))
    while len(parts) < 3:
        parts.append(0)
    return tuple(parts[:3])

required = sys.argv[1]
observed = md.version("paddleocr")
if version_tuple(observed) < version_tuple(required):
    raise RuntimeError(f"paddleocr {observed} is older than required {required}")
print(f"PPStructureV3 import ok; paddleocr={observed}")
PY
)" || {
    echo "PaddleOCR check failed. Expected: ${paddleocr_python_bin} can import paddleocr.PPStructureV3 with paddleocr >= ${required_paddleocr_version}" >&2
    echo "${paddleocr_output}" >&2
    exit 1
  }
  paddleocr_status="passed"
  echo "PaddleOCR: ${paddleocr_output}"
  if env_truthy "${INGEST_GATE_PADDLEOCR_SMOKE:-}"; then
    paddleocr_smoke_output="$("${paddleocr_python_bin}" - <<'PY' 2>&1
import os
import tempfile
from pathlib import Path
os.environ.setdefault("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True")
from paddleocr import PPStructureV3

pdf = b"""%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >> endobj
4 0 obj << /Length 74 >> stream
BT /F1 18 Tf 72 720 Td (PaddleOCR Smoke Resume) Tj T* (Company: Demo Corp) Tj ET
endstream endobj
5 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj
trailer << /Root 1 0 R >>
%%EOF
"""
path = Path(tempfile.gettempdir()) / "aiv3-paddleocr-gate-smoke.pdf"
path.write_bytes(pdf)
pipeline = PPStructureV3(
    ocr_version="PP-OCRv6",
    text_detection_model_name="PP-OCRv6_medium_det",
    text_recognition_model_name="PP-OCRv6_medium_rec",
    use_formula_recognition=False,
    use_chart_recognition=False,
    use_seal_recognition=False,
)
results = list(pipeline.predict(input=str(path)))
print(f"PPStructureV3 smoke pages={len(results)}")
PY
)" || {
      echo "PaddleOCR smoke failed. Expected: ${paddleocr_python_bin} can run PPStructureV3.predict on a tiny PDF" >&2
      echo "${paddleocr_smoke_output}" >&2
      exit 1
    }
    paddleocr_smoke_status="passed"
    echo "PaddleOCR smoke: ${paddleocr_smoke_output}"
  fi
else
  echo "PaddleOCR: skipped (not enabled)"
fi

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

INGEST_GATE_REPO_ROOT="${repo_root}" \
INGEST_GATE_HEAD="${head_short}" \
INGEST_GATE_STARTED_AT="${started_at}" \
INGEST_GATE_FINISHED_AT="${finished_at}" \
INGEST_GATE_PYTHON_BIN="${PYTHON_BIN}" \
INGEST_GATE_MARKITDOWN_VERSION="${markitdown_version_output}" \
INGEST_GATE_REQUIRED_MARKITDOWN_VERSION="${required_markitdown_version}" \
INGEST_GATE_REQUIRED_PADDLEOCR_VERSION="${required_paddleocr_version}" \
INGEST_GATE_DOCUMENT_PADDLEOCR_ENABLED="${DOCUMENT_PADDLEOCR_ENABLED:-}" \
INGEST_GATE_DOCUMENT_PDF_PARSE_ENGINE="${DOCUMENT_PDF_PARSE_ENGINE:-}" \
INGEST_GATE_DOCUMENT_PADDLEOCR_PYTHON_BIN="${DOCUMENT_PADDLEOCR_PYTHON_BIN:-}" \
INGEST_GATE_PADDLEOCR_REQUIRED="${paddleocr_required}" \
INGEST_GATE_PADDLEOCR_STATUS="${paddleocr_status}" \
INGEST_GATE_PADDLEOCR_OUTPUT="${paddleocr_output}" \
INGEST_GATE_PADDLEOCR_PYTHON_BIN="${paddleocr_python_bin}" \
INGEST_GATE_PADDLEOCR_SMOKE_STATUS="${paddleocr_smoke_status}" \
INGEST_GATE_PADDLEOCR_SMOKE_OUTPUT="${paddleocr_smoke_output}" \
node >"${report_json}" <<'NODE'
const fs = require("fs");
const paddleocrRequired = process.env.INGEST_GATE_PADDLEOCR_REQUIRED === "true";
const paddleocrStatus = process.env.INGEST_GATE_PADDLEOCR_STATUS || "skipped";
const checks = [
  { name: "PYTHON_BIN is executable or resolvable", status: "passed" },
  { name: "PYTHON_BIN -m markitdown --version exits successfully", status: "passed" },
  { name: "MarkItDown version matches pinned fallback version", status: "passed" }
];
if (paddleocrStatus !== "skipped") {
  checks.push({ name: "PaddleOCR runtime can import PPStructureV3 and meets PP-OCRv6 version floor", status: paddleocrStatus });
}
const paddleocrSmokeStatus = process.env.INGEST_GATE_PADDLEOCR_SMOKE_STATUS || "skipped";
if (paddleocrSmokeStatus !== "skipped") {
  checks.push({ name: "PaddleOCR PPStructureV3 can predict a tiny PDF", status: paddleocrSmokeStatus });
}
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
    optional_structured_pdf_parser: "PaddleOCR PP-StructureV3 is the default PDF parser when DOCUMENT_PADDLEOCR_PYTHON_BIN is configured; use DOCUMENT_PADDLEOCR_ENABLED=false or DOCUMENT_PDF_PARSE_ENGINE=native_first to opt out.",
    paddleocr_version_guard: "PaddleOCR runtime must be new enough for PP-OCRv6 model selection.",
    deployment_guard: "Deployment target must pass PYTHON_BIN -m markitdown --version with the pinned version before ingest fallback is considered ready."
  },
  python_bin: process.env.INGEST_GATE_PYTHON_BIN,
  required_markitdown_version: process.env.INGEST_GATE_REQUIRED_MARKITDOWN_VERSION,
  required_paddleocr_version: process.env.INGEST_GATE_REQUIRED_PADDLEOCR_VERSION,
  observed_markitdown_version: process.env.INGEST_GATE_MARKITDOWN_VERSION,
  paddleocr: {
    required: paddleocrRequired,
    document_paddleocr_enabled: process.env.INGEST_GATE_DOCUMENT_PADDLEOCR_ENABLED || "",
    document_pdf_parse_engine: process.env.INGEST_GATE_DOCUMENT_PDF_PARSE_ENGINE || "",
    document_paddleocr_python_bin: process.env.INGEST_GATE_DOCUMENT_PADDLEOCR_PYTHON_BIN || "",
    python_bin: process.env.INGEST_GATE_PADDLEOCR_PYTHON_BIN || "",
    check_status: paddleocrStatus,
    output: process.env.INGEST_GATE_PADDLEOCR_OUTPUT || "",
    smoke_status: paddleocrSmokeStatus,
    smoke_output: process.env.INGEST_GATE_PADDLEOCR_SMOKE_OUTPUT || ""
  },
  checks,
  notes: [
    "This gate is non-destructive and does not parse customer documents.",
    "Run on deployment targets after environment setup and before relying on MarkItDown fallback parsing.",
    "The PaddleOCR check validates package import and version readiness; first real parse may still need model files and adequate timeout.",
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
  `- Required PaddleOCR: ${report.required_paddleocr_version}`,
  `- Observed MarkItDown: ${report.observed_markitdown_version}`,
  `- PaddleOCR required: ${report.paddleocr.required}`,
  `- PaddleOCR Python: ${report.paddleocr.python_bin}`,
  `- PaddleOCR check: ${report.paddleocr.check_status}`,
  `- PaddleOCR smoke: ${report.paddleocr.smoke_status}`,
  "",
  "## Contract",
  "",
  `- Primary parser path: ${report.contract.primary_parser_path}`,
  `- Fallback parser: ${report.contract.fallback_parser}`,
  `- Optional structured PDF parser: ${report.contract.optional_structured_pdf_parser}`,
  `- PaddleOCR version guard: ${report.contract.paddleocr_version_guard}`,
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
