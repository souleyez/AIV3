#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

cases_path="${repo_root}/fixtures/retrieval-quality/cases.jsonl"
report_dir="${RETRIEVAL_QUALITY_SMOKE_REPORT_DIR:-${repo_root}/target/retrieval-quality-smoke}"
mode="smoke"
base_url=""
results_path=""
require_metrics="false"
fixture_policy="baseline"
required_case_count="30"
required_categories=$'deep_old_chunk\nselected_document_scope\nowner_scope\ncjk_phrase\ndatabase_topn\nrow_identity\nmixed_database_document'
cargo_bin="${CARGO_BIN:-cargo}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --baseline)
      mode="baseline"
      shift
      ;;
    --base-url)
      base_url="${2:-}"
      shift 2
      ;;
    --cases)
      cases_path="${2:-}"
      shift 2
      ;;
    --report-dir)
      report_dir="${2:-}"
      shift 2
      ;;
    --results-jsonl)
      results_path="${2:-}"
      shift 2
      ;;
    --require-metrics)
      require_metrics="true"
      shift
      ;;
    --live-subset)
      fixture_policy="live_subset"
      required_case_count="1"
      required_categories=""
      shift
      ;;
    -h|--help)
      cat <<'EOF'
Usage: bash scripts/run-retrieval-quality-smoke.sh [--baseline] [--base-url URL] [--cases PATH] [--report-dir PATH] [--results-jsonl PATH] [--require-metrics] [--live-subset]

Validates retrieval-quality fixtures and writes JSON/Markdown receipts.
By default it runs targeted cargo contract tests. Set
RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true to generate a fixture-only receipt.
Pass --results-jsonl to compute Recall/MRR/citation/answer/latency metrics from
one live result row per fixture case. Pass --require-metrics to fail when live
results are missing, malformed, incomplete, or contain permission leaks.
Pass --live-subset for deployment-target live probes that intentionally use a
small case file instead of the full 30+ case baseline coverage matrix.
EOF
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! -f "${cases_path}" ]]; then
  echo "cases file not found: ${cases_path}" >&2
  exit 1
fi

if [[ -n "${results_path}" && ! -f "${results_path}" ]]; then
  echo "results JSONL file not found: ${results_path}" >&2
  exit 1
fi

if [[ "${require_metrics}" == "true" && -z "${results_path}" ]]; then
  echo "--require-metrics requires --results-jsonl PATH" >&2
  exit 1
fi

if ! command -v "${cargo_bin}" >/dev/null 2>&1; then
  for candidate in "${HOME:-}/.cargo/bin/cargo" "/opt/homebrew/bin/cargo" "/usr/local/bin/cargo"; do
    if [[ -x "${candidate}" ]]; then
      cargo_bin="${candidate}"
      break
    fi
  done
fi

mkdir -p "${report_dir}"
report_basename="retrieval-quality-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
head_short="$(git rev-parse --short HEAD)"

checks=()

run_check() {
  local name="$1"
  shift
  echo ""
  echo "== ${name} =="
  "$@"
  checks+=("${name}")
}

skip_cargo="${RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO:-false}"
if [[ ! "${skip_cargo}" =~ ^([Tt][Rr][Uu][Ee]|1|[Yy][Ee][Ss]|[Oo][Nn])$ ]]; then
  if ! command -v "${cargo_bin}" >/dev/null 2>&1 && [[ ! -x "${cargo_bin}" ]]; then
    echo "cargo was not found. Install Rust or set CARGO_BIN=/path/to/cargo." >&2
    exit 1
  fi
  run_check "retrieval search API shape and ranking" \
    "${cargo_bin}" test -p platform-api search_dataset_retrieval_returns_ranked_hits_with_document_ids --lib
  run_check "retrieval deep chunk scan baseline" \
    "${cargo_bin}" test -p platform-api retrieval_search_scan_limit_covers_deep_chunks_in_large_documents --lib
  run_check "retrieval CJK phrase ranking" \
    "${cargo_bin}" test -p platform-api select_retrieval_evidence_ids_for_prompt_supports_chinese_terms --lib
  run_check "postgres lexical deep-old recall" \
    "${cargo_bin}" test -p platform-api postgres_lexical_retrieval_search_recalls_deep_old_chunk_beyond_latest_window --lib
  run_check "postgres lexical scope filters" \
    "${cargo_bin}" test -p platform-api postgres_lexical_retrieval_search_prefers_cjk_phrase_match --lib
  run_check "database aggregate heuristics" \
    "${cargo_bin}" test -p platform-api database_aggregate_heuristics --lib
else
  checks+=("cargo checks skipped by RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO")
fi

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

SMOKE_CASES_PATH="${cases_path}" \
SMOKE_REPORT_JSON="${report_json}" \
SMOKE_REPORT_MD="${report_md}" \
SMOKE_REPO_ROOT="${repo_root}" \
SMOKE_HEAD="${head_short}" \
SMOKE_MODE="${mode}" \
SMOKE_BASE_URL="${base_url}" \
SMOKE_RESULTS_JSONL="${results_path}" \
SMOKE_REQUIRE_METRICS="${require_metrics}" \
SMOKE_FIXTURE_POLICY="${fixture_policy}" \
SMOKE_REQUIRED_CASE_COUNT="${required_case_count}" \
SMOKE_REQUIRED_CATEGORIES="${required_categories}" \
SMOKE_STARTED_AT="${started_at}" \
SMOKE_FINISHED_AT="${finished_at}" \
SMOKE_SKIP_CARGO="${skip_cargo}" \
SMOKE_CHECKS="$(printf '%s\n' "${checks[@]}")" \
python3 <<'PY'
import json
import math
import os
import re
from collections import Counter
from pathlib import Path

cases_path = Path(os.environ["SMOKE_CASES_PATH"])
cases = []
with cases_path.open("r", encoding="utf-8") as handle:
    for line_no, line in enumerate(handle, 1):
        line = line.strip()
        if not line:
            continue
        try:
            case = json.loads(line)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"{cases_path}:{line_no}: invalid JSON: {exc}") from exc
        case["line_no"] = line_no
        cases.append(case)

required_categories = {
    category.strip()
    for category in os.environ.get("SMOKE_REQUIRED_CATEGORIES", "").splitlines()
    if category.strip()
}
required_case_count = int(os.environ.get("SMOKE_REQUIRED_CASE_COUNT", "30"))
ids = [case.get("id") for case in cases]
duplicate_ids = sorted({case_id for case_id in ids if ids.count(case_id) > 1})
categories = Counter(case.get("category") for case in cases)
missing_categories = sorted(required_categories.difference(categories))
missing_required_fields = []
for case in cases:
    for field in [
        "id",
        "category",
        "dataset_key",
        "prompt",
        "expected_sources",
        "expected_answer_patterns",
        "forbidden_sources",
        "forbidden_answer_patterns",
        "required_supply_types",
    ]:
        if field not in case:
            missing_required_fields.append({"id": case.get("id"), "line": case["line_no"], "field": field})

fixture_ready = (
    len(cases) >= required_case_count
    and not duplicate_ids
    and not missing_categories
    and not missing_required_fields
)

def as_list(value):
    return value if isinstance(value, list) else []

def source_value(value):
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        for key in ["source_id", "source", "document_id", "id"]:
            raw = value.get(key)
            if raw is not None:
                return str(raw)
    return ""

def source_matches(actual, expected):
    actual = str(actual or "")
    expected = str(expected or "")
    if not actual or not expected:
        return False
    return (
        actual == expected
        or actual.startswith(expected.rstrip("/") + "/")
        or expected in actual
    )

def text_matches(pattern, text):
    pattern = str(pattern or "")
    text = str(text or "")
    if not pattern:
        return False
    try:
        return re.search(pattern, text, flags=re.IGNORECASE) is not None
    except re.error:
        return pattern.lower() in text.lower()

def p95(values):
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, math.ceil(0.95 * len(ordered)) - 1)
    return ordered[index]

def ranked_sources(hits):
    normalized = []
    for index, hit in enumerate(as_list(hits), 1):
        rank = hit.get("rank") if isinstance(hit, dict) else None
        try:
            rank = int(rank)
        except (TypeError, ValueError):
            rank = index
        normalized.append((rank, source_value(hit)))
    normalized.sort(key=lambda item: item[0])
    return [source for _, source in normalized]

def evaluate_results(cases):
    results_path_value = os.environ.get("SMOKE_RESULTS_JSONL", "")
    require_metrics = os.environ.get("SMOKE_REQUIRE_METRICS", "false") == "true"
    default_metrics = {
        "recall_at_20": None,
        "mrr_at_20": None,
        "citation_accuracy": None,
        "answer_pattern_match_rate": None,
        "p95_latency_ms": None,
        "recorded": False,
        "required": require_metrics,
        "ready": False,
        "results_path": results_path_value or None,
        "result_case_count": 0,
        "missing_result_case_ids": [],
        "extra_result_case_ids": [],
        "duplicate_result_case_ids": [],
        "invalid_result_rows": [],
        "permission_leak_count": 0,
        "permission_leaks": [],
    }
    if not results_path_value:
        return default_metrics

    results_path = Path(results_path_value)
    results = {}
    duplicate_result_case_ids = set()
    invalid_result_rows = []
    with results_path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            try:
                result = json.loads(line)
            except json.JSONDecodeError as exc:
                invalid_result_rows.append({"line": line_no, "error": f"invalid JSON: {exc}"})
                continue
            case_id = result.get("case_id")
            if not isinstance(case_id, str) or not case_id:
                invalid_result_rows.append({"line": line_no, "error": "missing case_id"})
                continue
            if case_id in results:
                duplicate_result_case_ids.add(case_id)
            results[case_id] = result

    case_ids = [case.get("id") for case in cases]
    known_case_ids = set(case_ids)
    missing_result_case_ids = sorted(case_id for case_id in case_ids if case_id not in results)
    extra_result_case_ids = sorted(set(results).difference(known_case_ids))
    recall_scores = []
    reciprocal_ranks = []
    citation_scores = []
    answer_scores = []
    latencies = []
    permission_leaks = []

    for case in cases:
        case_id = case.get("id")
        result = results.get(case_id)
        if result is None:
            recall_scores.append(0.0)
            reciprocal_ranks.append(0.0)
            citation_scores.append(0.0)
            answer_scores.append(0.0)
            continue

        answer = result.get("answer")
        hits = result.get("hits")
        citations = result.get("citations")
        latency_ms = result.get("latency_ms")
        if not isinstance(answer, str):
            invalid_result_rows.append({"case_id": case_id, "error": "answer must be a string"})
            answer = ""
        if not isinstance(hits, list):
            invalid_result_rows.append({"case_id": case_id, "error": "hits must be an array"})
            hits = []
        if not isinstance(citations, list):
            invalid_result_rows.append({"case_id": case_id, "error": "citations must be an array"})
            citations = []
        if not isinstance(latency_ms, (int, float)) or latency_ms < 0:
            invalid_result_rows.append({"case_id": case_id, "error": "latency_ms must be a non-negative number"})
        else:
            latencies.append(float(latency_ms))

        expected_sources = [str(value) for value in as_list(case.get("expected_sources"))]
        forbidden_sources = [str(value) for value in as_list(case.get("forbidden_sources"))]
        expected_answer_patterns = [str(value) for value in as_list(case.get("expected_answer_patterns"))]
        forbidden_answer_patterns = [str(value) for value in as_list(case.get("forbidden_answer_patterns"))]
        top20_sources = ranked_sources(hits)[:20]
        citation_sources = [source_value(citation) for citation in citations]

        first_expected_rank = None
        for rank, source in enumerate(top20_sources, 1):
            if any(source_matches(source, expected) for expected in expected_sources):
                first_expected_rank = rank
                break
        recall_scores.append(1.0 if first_expected_rank is not None else 0.0)
        reciprocal_ranks.append(1.0 / first_expected_rank if first_expected_rank else 0.0)

        citation_ok = bool(citation_sources) and all(
            any(source_matches(source, expected) for expected in expected_sources)
            for source in citation_sources
        )
        citation_scores.append(1.0 if citation_ok else 0.0)

        answer_ok = all(text_matches(pattern, answer) for pattern in expected_answer_patterns) and not any(
            text_matches(pattern, answer) for pattern in forbidden_answer_patterns
        )
        answer_scores.append(1.0 if answer_ok else 0.0)

        for forbidden in forbidden_sources:
            if any(source_matches(source, forbidden) for source in top20_sources + citation_sources):
                permission_leaks.append({"case_id": case_id, "type": "forbidden_source", "value": forbidden})
        for pattern in forbidden_answer_patterns:
            if text_matches(pattern, answer):
                permission_leaks.append({"case_id": case_id, "type": "forbidden_answer_pattern", "value": pattern})

    denominator = max(1, len(cases))
    metrics_ready = (
        not missing_result_case_ids
        and not duplicate_result_case_ids
        and not invalid_result_rows
        and not permission_leaks
    )
    return {
        "recall_at_20": round(sum(recall_scores) / denominator, 6),
        "mrr_at_20": round(sum(reciprocal_ranks) / denominator, 6),
        "citation_accuracy": round(sum(citation_scores) / denominator, 6),
        "answer_pattern_match_rate": round(sum(answer_scores) / denominator, 6),
        "p95_latency_ms": p95(latencies),
        "recorded": True,
        "required": require_metrics,
        "ready": metrics_ready,
        "results_path": str(results_path),
        "result_case_count": len(results),
        "missing_result_case_ids": missing_result_case_ids,
        "extra_result_case_ids": extra_result_case_ids,
        "duplicate_result_case_ids": sorted(duplicate_result_case_ids),
        "invalid_result_rows": invalid_result_rows,
        "permission_leak_count": len(permission_leaks),
        "permission_leaks": permission_leaks,
    }

metrics = evaluate_results(cases)
checks = [line for line in os.environ.get("SMOKE_CHECKS", "").splitlines() if line]
base_url = os.environ.get("SMOKE_BASE_URL", "")
report = {
    "smoke": "retrieval-quality",
    "mode": os.environ["SMOKE_MODE"],
    "ready": fixture_ready and (not metrics["required"] or metrics["ready"]),
    "repository": os.environ["SMOKE_REPO_ROOT"],
    "head": os.environ["SMOKE_HEAD"],
    "started_at": os.environ["SMOKE_STARTED_AT"],
    "finished_at": os.environ["SMOKE_FINISHED_AT"],
    "cases_path": str(cases_path),
    "fixture_policy": os.environ.get("SMOKE_FIXTURE_POLICY", "baseline"),
    "case_count": len(cases),
    "required_case_count": required_case_count,
    "categories": dict(sorted(categories.items())),
    "missing_categories": missing_categories,
    "duplicate_ids": duplicate_ids,
    "missing_required_fields": missing_required_fields,
    "fixture_ready": fixture_ready,
    "permission_leak_count": metrics["permission_leak_count"],
    "api_smoke": {
        "requested": bool(base_url),
        "base_url": base_url or None,
        "status": "not_run",
        "reason": "No public retrieval-search HTTP contract is exercised by this fixture smoke yet."
    },
    "metrics": metrics,
    "checks": [{"name": name, "status": "passed"} for name in checks],
}

Path(os.environ["SMOKE_REPORT_JSON"]).write_text(
    json.dumps(report, ensure_ascii=False, indent=2) + "\n",
    encoding="utf-8",
)

md_lines = [
    "# Retrieval Quality Smoke",
    "",
    f"- Status: {'passed' if report['ready'] else 'failed'}",
    f"- Mode: {report['mode']}",
    f"- Repository: {report['repository']}",
    f"- HEAD: {report['head']}",
    f"- Cases: {report['case_count']}",
    f"- Permission leak count: {report['permission_leak_count']}",
    f"- Metrics recorded: {report['metrics']['recorded']}",
    "",
    "## Category Coverage",
    "",
]
for category, count in sorted(categories.items()):
    md_lines.append(f"- {category}: {count}")
md_lines.extend(["", "## Checks", ""])
for check in report["checks"]:
    md_lines.append(f"- {check['status']}: {check['name']}")
if missing_categories or duplicate_ids or missing_required_fields:
    md_lines.extend(["", "## Fixture Issues", ""])
    for category in missing_categories:
        md_lines.append(f"- missing category: {category}")
    for case_id in duplicate_ids:
        md_lines.append(f"- duplicate id: {case_id}")
    for item in missing_required_fields:
        md_lines.append(f"- missing field line {item['line']}: {item['field']}")
md_lines.extend([
    "",
    "## API Metrics",
    "",
])
if report["metrics"]["recorded"]:
    md_lines.extend([
        f"- Results path: {report['metrics']['results_path']}",
        f"- Results ready: {report['metrics']['ready']}",
        f"- Recall@20: {report['metrics']['recall_at_20']}",
        f"- MRR@20: {report['metrics']['mrr_at_20']}",
        f"- Citation accuracy: {report['metrics']['citation_accuracy']}",
        f"- Answer pattern match rate: {report['metrics']['answer_pattern_match_rate']}",
        f"- p95 latency ms: {report['metrics']['p95_latency_ms']}",
        f"- Missing result cases: {len(report['metrics']['missing_result_case_ids'])}",
        f"- Invalid result rows: {len(report['metrics']['invalid_result_rows'])}",
        f"- Permission leaks: {report['metrics']['permission_leak_count']}",
        "",
    ])
else:
    md_lines.extend([
        "This smoke currently validates fixture coverage and targeted retrieval contract tests.",
        "Recall/MRR/citation/latency metrics remain unset until --results-jsonl is provided.",
        "",
    ])
Path(os.environ["SMOKE_REPORT_MD"]).write_text("\n".join(md_lines), encoding="utf-8")

if report["metrics"]["required"] and not report["metrics"]["ready"]:
    raise SystemExit(
        "metrics required but not ready: "
        f"missing={len(report['metrics']['missing_result_case_ids'])}, "
        f"invalid={len(report['metrics']['invalid_result_rows'])}, "
        f"duplicates={len(report['metrics']['duplicate_result_case_ids'])}, "
        f"leaks={report['metrics']['permission_leak_count']}"
    )
PY

echo ""
echo "Retrieval quality smoke receipt written:"
echo "- ${report_json}"
echo "- ${report_md}"
