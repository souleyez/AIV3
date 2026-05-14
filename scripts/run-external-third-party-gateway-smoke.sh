#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

if [[ -f /etc/aiv3/aiv3.env && -z "${PLATFORM_DATABASE_URL:-}" ]]; then
  echo "Refusing to load /etc/aiv3/aiv3.env for destructive smoke tests." >&2
  echo "Set PLATFORM_DATABASE_URL to a disposable test database instead." >&2
  exit 1
fi

if [[ -z "${PLATFORM_DATABASE_URL:-}" ]]; then
  echo "PLATFORM_DATABASE_URL is required for this smoke test." >&2
  echo "Use a disposable test database; this smoke test resets its database." >&2
  exit 1
fi

if [[ "${PLATFORM_DATABASE_URL}" == *"/ai_data_platform_v3"* && "${PLATFORM_DATABASE_URL}" != *"test"* ]]; then
  echo "Refusing to run destructive smoke tests against ai_data_platform_v3." >&2
  echo "Point PLATFORM_DATABASE_URL at a disposable test database." >&2
  exit 1
fi

port="${EXTERNAL_THIRD_PARTY_MOCK_PORT:-43180}"
host="${EXTERNAL_THIRD_PARTY_MOCK_HOST:-127.0.0.1}"
base_url="http://${host}:${port}"
log_file="${TMPDIR:-/tmp}/external-third-party-mock-gateway-${port}.log"
report_dir="${EXTERNAL_THIRD_PARTY_READINESS_REPORT_DIR:-${repo_root}/target/external-third-party-readiness}"
handoff_manifest="${EXTERNAL_THIRD_PARTY_HANDOFF_MANIFEST:-${repo_root}/docs/integrations/third-party-handoff.sample.json}"

export EXTERNAL_THIRD_PARTY_MOCK_HOST="${host}"
export EXTERNAL_THIRD_PARTY_MOCK_PORT="${port}"
export EXTERNAL_THIRD_PARTY_MOCK_BEARER_TOKEN="${EXTERNAL_THIRD_PARTY_MOCK_BEARER_TOKEN:-dispatch-token}"
export EXTERNAL_THIRD_PARTY_MOCK_SIGNING_SECRET="${EXTERNAL_THIRD_PARTY_MOCK_SIGNING_SECRET:-dispatch-secret}"
export EXTERNAL_THIRD_PARTY_MOCK_REQUEST_ID="${EXTERNAL_THIRD_PARTY_MOCK_REQUEST_ID:-gateway-req-001}"
export EXTERNAL_THIRD_PARTY_MOCK_DISPATCH_URL="${base_url}/third-party/actions?tenant=tenant-ext-001"
export EXTERNAL_THIRD_PARTY_MOCK_RESULT_HELPER_URL="${base_url}/__mock/send-action-result"

node "${script_dir}/external-third-party-mock-gateway.mjs" >"${log_file}" 2>&1 &
gateway_pid="$!"
trap 'kill "${gateway_pid}" >/dev/null 2>&1 || true' EXIT

for _ in $(seq 1 50); do
  if curl -fsS "${base_url}/__mock/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

curl -fsS "${base_url}/__mock/health" >/dev/null

echo "External third-party gateway smoke started"
echo "Gateway: ${base_url}"
echo "Repository: ${repo_root}"
echo "Handoff manifest: ${handoff_manifest}"
printf "HEAD: "
git rev-parse --short HEAD

cargo test -p platform-api external_action_dispatch_posts_to_external_mock_gateway_from_env --lib -- --nocapture
cargo test -p platform-api external_action_result_callback_records_redacted_summary --lib -- --nocapture
cargo test -p platform-api external_action_gateway_posts_result_callback_to_v3_from_env --lib -- --nocapture

requests_json="$(curl -fsS "${base_url}/__mock/requests")"
printf '%s' "${requests_json}" | node -e '
const fs = require("fs");
const data = JSON.parse(fs.readFileSync(0, "utf8"));
if (!Array.isArray(data.requests) || data.requests.length !== 1) {
  throw new Error(`expected exactly one mock gateway request, got ${data.requests?.length ?? "none"}`);
}
const request = data.requests[0];
for (const key of ["bearer_valid", "signature_valid", "body_hash_valid", "requester_sender_present"]) {
  if (request[key] !== true) {
    throw new Error(`mock gateway request failed ${key}: ${JSON.stringify(request)}`);
  }
}
if (request.raw_arguments_included || request.contains_forbidden_text) {
  throw new Error(`mock gateway saw unsafe payload: ${JSON.stringify(request)}`);
}
console.log("Mock gateway accepted signed dispatch:", JSON.stringify({
  action_id: request.action_id,
  action_type: request.action_type,
  bearer_valid: request.bearer_valid,
  signature_valid: request.signature_valid,
  body_hash_valid: request.body_hash_valid
}));
'

callbacks_json="$(curl -fsS "${base_url}/__mock/callbacks")"
printf '%s' "${callbacks_json}" | node -e '
const fs = require("fs");
const data = JSON.parse(fs.readFileSync(0, "utf8"));
if (!Array.isArray(data.callbacks) || data.callbacks.length !== 1) {
  throw new Error(`expected exactly one mock gateway callback, got ${data.callbacks?.length ?? "none"}`);
}
const callback = data.callbacks[0];
if (callback.callback_status !== 200 || callback.response_accepted !== true) {
  throw new Error(`mock gateway callback failed: ${JSON.stringify(callback)}`);
}
if (callback.contains_forbidden_text) {
  throw new Error(`mock gateway callback response leaked unsafe text: ${JSON.stringify(callback)}`);
}
console.log("Mock gateway posted action result callback:", JSON.stringify({
  callback_status: callback.callback_status,
  response_accepted: callback.response_accepted,
  action_id: callback.response_summary?.action_id,
  status: callback.response_summary?.status
}));
'

mkdir -p "${report_dir}"
report_basename="external-third-party-readiness-$(date -u +%Y%m%dT%H%M%SZ)"
readiness_report="$(
  EXTERNAL_THIRD_PARTY_REQUESTS_JSON="${requests_json}" \
  EXTERNAL_THIRD_PARTY_CALLBACKS_JSON="${callbacks_json}" \
  node "${repo_root}/tools/external-third-party-readiness-report.mjs" \
    --gateway "${base_url}" \
    --repository "${repo_root}" \
    --head "$(git rev-parse --short HEAD)" \
    --handoffManifest "${handoff_manifest}" \
    --outDir "${report_dir}" \
    --basename "${report_basename}"
)"
printf 'External third-party readiness report: %s\n' "${readiness_report}"

echo "OK external third-party gateway smoke completed."
