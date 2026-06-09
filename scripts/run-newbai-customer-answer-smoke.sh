#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

node_bin="${NODE_BIN:-node}"
output_dir="${NEWBAI_CUSTOMER_ANSWER_SMOKE_OUTPUT_DIR:-${repo_root}/target/newbai-customer-answer-smoke}"
fixture_path="${NEWBAI_CUSTOMER_ANSWER_FIXTURE:-${repo_root}/fixtures/newbai-customer-answer/cases.jsonl}"

if ! command -v "${node_bin}" >/dev/null 2>&1; then
  echo "node was not found. Install Node.js or set NODE_BIN=/path/to/node." >&2
  exit 1
fi

args=(--self-test --pretty --fixture "${fixture_path}" --output-dir "${output_dir}")
if [[ -n "${NEWBAI_CUSTOMER_ANSWER_RESULTS_JSONL:-}" ]]; then
  args+=(--results-jsonl "${NEWBAI_CUSTOMER_ANSWER_RESULTS_JSONL}")
fi

echo "NewBai customer answer smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: $(git rev-parse --short HEAD)"
echo "Fixture: ${fixture_path}"
echo "Output directory: ${output_dir}"
if [[ -n "${NEWBAI_CUSTOMER_ANSWER_RESULTS_JSONL:-}" ]]; then
  echo "Results JSONL: ${NEWBAI_CUSTOMER_ANSWER_RESULTS_JSONL}"
else
  echo "Results JSONL: fixture sample_result only"
fi

"${node_bin}" scripts/smoke/newbai-customer-answer.mjs "${args[@]}"

echo "OK newbai-customer-answer smoke completed."
