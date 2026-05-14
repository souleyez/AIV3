#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

if [[ -f /etc/aiv3/aiv3.env && -z "${PLATFORM_DATABASE_URL:-}" ]]; then
  set -a
  # shellcheck disable=SC1091
  . /etc/aiv3/aiv3.env
  set +a
fi

port="${EXTERNAL_THIRD_PARTY_MOCK_PORT:-43180}"
host="${EXTERNAL_THIRD_PARTY_MOCK_HOST:-127.0.0.1}"
base_url="http://${host}:${port}"
log_file="${TMPDIR:-/tmp}/external-third-party-mock-gateway-${port}.log"

export EXTERNAL_THIRD_PARTY_MOCK_HOST="${host}"
export EXTERNAL_THIRD_PARTY_MOCK_PORT="${port}"
export EXTERNAL_THIRD_PARTY_MOCK_BEARER_TOKEN="${EXTERNAL_THIRD_PARTY_MOCK_BEARER_TOKEN:-dispatch-token}"
export EXTERNAL_THIRD_PARTY_MOCK_SIGNING_SECRET="${EXTERNAL_THIRD_PARTY_MOCK_SIGNING_SECRET:-dispatch-secret}"
export EXTERNAL_THIRD_PARTY_MOCK_REQUEST_ID="${EXTERNAL_THIRD_PARTY_MOCK_REQUEST_ID:-gateway-req-001}"
export EXTERNAL_THIRD_PARTY_MOCK_DISPATCH_URL="${base_url}/third-party/actions?tenant=tenant-ext-001"

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
printf "HEAD: "
git rev-parse --short HEAD

cargo test -p platform-api external_action_dispatch_posts_to_external_mock_gateway_from_env --lib -- --nocapture
cargo test -p platform-api external_action_result_callback_records_redacted_summary --lib -- --nocapture

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

echo "OK external third-party gateway smoke completed."
