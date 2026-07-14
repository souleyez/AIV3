#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1 && [[ -d "$HOME/.cargo/bin" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

if [[ "${1:-}" != "--no-credentials" || "$#" -ne 1 ]]; then
  echo "usage: $0 --no-credentials" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log_dir="$(mktemp -d "${TMPDIR:-/tmp}/datamax-semantic-smoke.XXXXXX")"
trap 'rm -rf "$log_dir"' EXIT
cd "$repo_root"

run_step() {
  local name="$1"
  shift
  if ! "$@" >"$log_dir/$name.log" 2>&1; then
    echo "dataset semantic smoke failed: $name" >&2
    sed -n '1,200p' "$log_dir/$name.log" >&2
    exit 1
  fi
}

run_step source_contract \
  cargo test -p platform-api \
  no_credential_smoke_all_source_kinds_share_one_business_safe_contract \
  --lib --quiet
run_step empty_unknown \
  cargo test -p platform-api \
  empty_unknown_fixture_keeps_technical_field_unresolved_and_out_of_headline \
  --lib --quiet
run_step access_and_projection \
  cargo test -p platform-api dataset_semantic_understanding_support --lib --quiet
run_step evidence_classes \
  cargo test -p platform-api semantic_relation_builder --lib --quiet
run_step web_contract \
  pnpm --dir apps/web exec node --test \
  app/lib/dataset-understanding-api.test.mjs \
  app/lib/dataset-understanding-graph.test.mjs

printf '%s\n' '{"status":"passed","schema_version":"1.0.0","source_kind_count":6,"smoke_case_count":8,"credentials_used":false,"safe_labels":["业务对象","关键字段","已确认事实","推断关系"]}'
