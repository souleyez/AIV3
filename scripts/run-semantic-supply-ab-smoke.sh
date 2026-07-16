#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
node_bin="${NODE_BIN:-node}"
output_dir="${SEMANTIC_SUPPLY_AB_OUTPUT_DIR:-${repo_root}/target/semantic-supply-ab-smoke}"

result_args=()
result_path_count=0
for path in \
  "${SEMANTIC_SUPPLY_A_RESULTS:-}" \
  "${SEMANTIC_SUPPLY_B_RESULTS:-}" \
  "${SEMANTIC_SUPPLY_C_RESULTS:-}"; do
  if [[ -n "${path}" ]]; then
    result_path_count=$((result_path_count + 1))
  fi
done
if [[ "${result_path_count}" -ne 0 && "${result_path_count}" -ne 3 ]]; then
  echo "SEMANTIC_SUPPLY_A_RESULTS, SEMANTIC_SUPPLY_B_RESULTS and SEMANTIC_SUPPLY_C_RESULTS must be set together" >&2
  exit 2
fi
runtime_input_arg_present=false
for arg in "$@"; do
  if [[ "${arg}" == "--runtime-input" ]]; then
    runtime_input_arg_present=true
    break
  fi
done
if [[ "${result_path_count}" -eq 3 \
  && -z "${SEMANTIC_SUPPLY_RUNTIME_INPUT:-}" \
  && "${runtime_input_arg_present}" != true ]]; then
  echo "SEMANTIC_SUPPLY_RUNTIME_INPUT or --runtime-input is required with A/B/C results" >&2
  exit 2
fi
if [[ "${result_path_count}" -eq 3 ]]; then
  result_args+=(
    --a-results "${SEMANTIC_SUPPLY_A_RESULTS}"
    --b-results "${SEMANTIC_SUPPLY_B_RESULTS}"
    --c-results "${SEMANTIC_SUPPLY_C_RESULTS}"
  )
fi
if [[ -n "${SEMANTIC_SUPPLY_RUNTIME_INPUT:-}" ]]; then
  result_args+=(--runtime-input "${SEMANTIC_SUPPLY_RUNTIME_INPUT}")
fi
if [[ -n "${SEMANTIC_SUPPLY_ANSWER_RECEIPT:-}" ]]; then
  result_args+=(--answer-receipt "${SEMANTIC_SUPPLY_ANSWER_RECEIPT}")
fi

exec "${node_bin}" \
  "${repo_root}/scripts/smoke/semantic-supply-ab.mjs" \
  --fixture "${repo_root}/fixtures/semantic-supply-ab/cases.jsonl" \
  --output-dir "${output_dir}" \
  "${result_args[@]}" \
  "$@"
