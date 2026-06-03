#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

api_base="${MODEL_GATEWAY_SMOKE_API_BASE_URL:-${AIV3_API_BASE_URL:-http://127.0.0.1:8080}}"
connection_id="${MODEL_GATEWAY_SMOKE_CONNECTION_ID:-generic-chat-main}"
bearer_token="${MODEL_GATEWAY_SMOKE_BEARER:-}"
concurrency="${MODEL_GATEWAY_SMOKE_CONCURRENCY:-10}"
timeout_seconds="${MODEL_GATEWAY_SMOKE_TIMEOUT_SECONDS:-90}"
report_dir="${MODEL_GATEWAY_SMOKE_REPORT_DIR:-${repo_root}/target/model-gateway-rollout-smoke}"
report_basename="model-gateway-rollout-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
request_prefix="${report_dir}/${report_basename}-request"
url="${api_base%/}/v1/external/channels/${connection_id}/events/stream"

if ! command -v curl >/dev/null 2>&1; then
  echo "curl was not found." >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node was not found." >&2
  exit 1
fi

if ! [[ "${concurrency}" =~ ^[0-9]+$ ]] || [[ "${concurrency}" -lt 1 ]]; then
  echo "MODEL_GATEWAY_SMOKE_CONCURRENCY must be a positive integer." >&2
  exit 1
fi

mkdir -p "${report_dir}"

started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
head_short="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

echo "Model gateway rollout smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Endpoint: ${url}"
echo "Concurrency: ${concurrency}"
echo "Report directory: ${report_dir}"

make_payload() {
  local index="$1"
  SMOKE_INDEX="${index}" \
  SMOKE_RUN_ID="${report_basename}" \
  SMOKE_RECEIVED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  node <<'NODE'
const index = process.env.SMOKE_INDEX;
const runId = process.env.SMOKE_RUN_ID;
const payload = {
  platform: "generic_chat",
  tenant_external_id: process.env.MODEL_GATEWAY_SMOKE_TENANT_ID || "tenant-ext-001",
  bot_external_id: process.env.MODEL_GATEWAY_SMOKE_BOT_ID || "bot-v3",
  conversation_external_id: process.env.MODEL_GATEWAY_SMOKE_CONVERSATION_ID || `model-gateway-smoke-${runId}-${index}`,
  sender_external_id: process.env.MODEL_GATEWAY_SMOKE_SENDER_ID || "model-gateway-smoke-user",
  message_external_id: `model-gateway-smoke-${runId}-${index}`,
  message_type: "text",
  text: process.env.MODEL_GATEWAY_SMOKE_TEXT || "模型池 smoke test：请用一句话回答当前模型通道是否可用。",
  attachment_refs: [],
  idempotency_key: `model-gateway-smoke:${runId}:${index}`,
  received_at: process.env.SMOKE_RECEIVED_AT
};
process.stdout.write(JSON.stringify(payload));
NODE
}

curl_once() {
  local index="$1"
  local output_file="${request_prefix}-${index}.sse"
  local payload
  local http_code
  local auth_args=()
  payload="$(make_payload "${index}")"
  if [[ -n "${bearer_token}" ]]; then
    auth_args=(-H "Authorization: Bearer ${bearer_token}")
  fi

  http_code="$(
    curl -sS \
      --max-time "${timeout_seconds}" \
      -o "${output_file}" \
      -w "%{http_code}" \
      -X POST "${url}" \
      -H "Content-Type: application/json" \
      "${auth_args[@]}" \
      --data-binary "${payload}"
  )"

  if [[ "${http_code}" != "200" ]]; then
    echo "request ${index} returned HTTP ${http_code}; response saved to ${output_file}" >&2
    return 1
  fi
  if ! grep -q "event: external_channel.completed" "${output_file}"; then
    echo "request ${index} did not emit external_channel.completed; response saved to ${output_file}" >&2
    return 1
  fi
  if ! grep -Eq '"task_status"[[:space:]]*:[[:space:]]*"answered"' "${output_file}"; then
    echo "request ${index} did not complete with task_status=answered; response saved to ${output_file}" >&2
    return 1
  fi
}

echo ""
echo "== Warmup =="
curl_once "warmup"

echo ""
echo "== Concurrent smoke =="
pids=()
log_files=()
for index in $(seq 1 "${concurrency}"); do
  log_file="${request_prefix}-${index}.log"
  log_files+=("${log_file}")
  (
    curl_once "${index}"
  ) >"${log_file}" 2>&1 &
  pids+=("$!")
done

passed=0
failed=0
for offset in "${!pids[@]}"; do
  pid="${pids[${offset}]}"
  log_file="${log_files[${offset}]}"
  if wait "${pid}"; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    cat "${log_file}" >&2
  fi
done

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
ready=false
if [[ "${failed}" -eq 0 ]]; then
  ready=true
fi

SMOKE_REPO_ROOT="${repo_root}" \
SMOKE_HEAD="${head_short}" \
SMOKE_STARTED_AT="${started_at}" \
SMOKE_FINISHED_AT="${finished_at}" \
SMOKE_API_BASE="${api_base%/}" \
SMOKE_CONNECTION_ID="${connection_id}" \
SMOKE_CONCURRENCY="${concurrency}" \
SMOKE_PASSED="${passed}" \
SMOKE_FAILED="${failed}" \
SMOKE_READY="${ready}" \
node >"${report_json}" <<'NODE'
const report = {
  smoke: "model-gateway-rollout",
  ready: process.env.SMOKE_READY === "true",
  repository: process.env.SMOKE_REPO_ROOT,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  api_base: process.env.SMOKE_API_BASE,
  connection_id: process.env.SMOKE_CONNECTION_ID,
  concurrency: Number(process.env.SMOKE_CONCURRENCY),
  checks: [
    { name: "warmup external channel stream completed", status: "passed" },
    {
      name: "concurrent external channel streams completed with answered replies",
      status: process.env.SMOKE_READY === "true" ? "passed" : "failed",
      passed: Number(process.env.SMOKE_PASSED),
      failed: Number(process.env.SMOKE_FAILED)
    }
  ]
};
process.stdout.write(JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, "utf8"));
const lines = [
  "# Model Gateway Rollout Smoke",
  "",
  `- Status: ${report.ready ? "passed" : "failed"}`,
  `- Repository: ${report.repository}`,
  `- HEAD: ${report.head}`,
  `- API base: ${report.api_base}`,
  `- Connection: ${report.connection_id}`,
  `- Concurrency: ${report.concurrency}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  "",
  "## Checks",
  "",
  ...report.checks.map((check) => {
    const counts = check.passed === undefined ? "" : ` (${check.passed} passed, ${check.failed} failed)`;
    return `- ${check.status}: \`${check.name}\`${counts}`;
  }),
  "",
].join("\n");
process.stdout.write(lines);
NODE

echo ""
echo "Model gateway rollout smoke report: ${report_json}"
echo "Model gateway rollout smoke summary: ${report_md}"

if [[ "${failed}" -ne 0 ]]; then
  echo "FAILED model-gateway rollout smoke: ${failed} of ${concurrency} concurrent requests failed." >&2
  exit 1
fi

echo "OK model-gateway rollout smoke completed."
