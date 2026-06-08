# Retrieval Quality Smoke

This smoke is the Phase 0 baseline harness for the AIV3 RAG/retrieval supply
plan. It validates that the retrieval-quality fixture set covers the known
failure modes before DB-side lexical retrieval, pgvector, hybrid ranking, or
structured query planning are enabled.

## Command

```bash
bash scripts/run-retrieval-quality-smoke.sh --baseline
```

The script writes JSON and Markdown reports under:

```text
target/retrieval-quality-smoke/
```

For fixture-only validation:

```bash
RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true bash scripts/run-retrieval-quality-smoke.sh --baseline
```

## Current Coverage

- Deep old chunks that should not be lost by `latest-512` retrieval scanning.
- Selected-document scope and external temporary document ranges.
- Owner/private-document scope.
- Chinese business phrases such as `订单延期风险`、`取高机会`、`经营风险`.
- Database TopN and metric ranking prompts that must prefer `database_aggregate`.
- Row identity collapse cases that must not claim row-level completeness.
- Mixed database + parsed-document questions where aggregate rows answer the
  number and retrieval evidence supplies narrative support.

## Receipt Contract

The receipt records:

- `case_count`
- category coverage
- duplicate case IDs
- missing required fields
- `permission_leak_count`
- targeted cargo checks
- whether Recall/MRR/citation/latency metrics were recorded

The default harness does not claim Recall@20, MRR@20, citation accuracy, or p95
latency unless `--results-jsonl` is provided with authenticated live results.

## Live Result Evaluator

The evaluator accepts one JSONL row per fixture case through `--results-jsonl`.
The default smoke continues to report `metrics.recorded=false` unless real
result rows are provided. `--require-metrics` fails when result rows are missing,
malformed, incomplete, duplicated, or contain permission leaks.

Minimum row shape:

```json
{
  "case_id": "deep_old_chunk_contract_clause",
  "latency_ms": 842,
  "answer": "answer text",
  "hits": [
    {
      "rank": 1,
      "source_id": "document:contract-a:chunk-42",
      "document_id": "contract-a",
      "supply_type": "retrieval_evidence",
      "score": 0.93
    }
  ],
  "citations": [
    {
      "source_id": "document:contract-a:chunk-42",
      "quote": "short citation excerpt"
    }
  ]
}
```

Recorded metrics:

- Recall@20 from expected source hits.
- MRR@20 from the first expected source rank.
- Citation accuracy from allowed citation sources.
- Answer pattern match against expected and forbidden answer patterns.
- Permission leak count from forbidden hits, citations, or answer text.
- p95 latency from `latency_ms`.

## 2026-06-08 Local Receipt

- Command: `bash scripts/run-retrieval-quality-smoke.sh --baseline`
- Result: passed
- JSON report: `target/retrieval-quality-smoke/retrieval-quality-smoke-20260608T224648Z.json`
- Markdown report: `target/retrieval-quality-smoke/retrieval-quality-smoke-20260608T224648Z.md`
- Fixture count: 35
- Required count: 30
- Permission leak count: 0
- Covered categories:
  - `deep_old_chunk`
  - `selected_document_scope`
  - `owner_scope`
  - `cjk_phrase`
  - `database_topn`
  - `row_identity`
  - `mixed_database_document`
- NewBai/report-regression additions:
  - structured TopN for sales gap + traffic decline
  - mixed database + template narrative evidence
  - row identity guard for all-store detail requests

The smoke includes DB-side lexical recall beyond the legacy latest-window
candidate set, phrase ranking, selected-document scope, and owner scope
filtering before ranking. The deep-old check also runs local `EXPLAIN (FORMAT
JSON)` with `enable_seqscan=off` and verifies
`retrieval_evidences_search_terms_gin_idx` is present in the plan.

## 2026-06-08 Evaluator Self-Check

- Command: `RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true bash scripts/run-retrieval-quality-smoke.sh --baseline --results-jsonl target/retrieval-quality-smoke/synthetic-live-results.jsonl --require-metrics`
- Result: passed
- JSON report: `target/retrieval-quality-smoke/retrieval-quality-smoke-20260608T223542Z.json`
- Fixture count: 35
- Result case count: 35
- Recall@20: `1.0`
- MRR@20: `1.0`
- Citation accuracy: `1.0`
- Answer pattern match rate: `1.0`
- p95 latency ms: `134.0`
- Permission leak count: `0`

This is a synthetic evaluator self-check only. It proves the metric calculator
and `--require-metrics` gate work; it is not a live quality score for a real
authenticated dataset.

## 2026-06-09 8 Server Read-Only Preflight

- Scope: read-only live preflight; no GitHub pull, no service restart, no
  migration, no feature flag change.
- Host: `8.155.8.7`
- Hostname: `iZf8za4zfs8dtm3vfyzvj7Z`
- AIV3 repo: `/srv/aiv3/repo`
- Server repo HEAD: `6939a43abd93d5de68b944cb29322d0dff2fdfb0`
- Server branch: `main`
- Server worktree note: untracked `mode`
- Active services:
  - `aiv3-platform-api.service`
  - `aiv3-retrieval-worker.service`
  - `aiv3-web.service`
  - `postgresql-17.service`
  - `nginx.service`
  - `nats-server.service`
- Local health checks:
  - `http://127.0.0.1:3000/healthz`: ok
  - `http://127.0.0.1:3000/readyz`: ready
- Public route checks:
  - `https://v3.elepcloud.com/`: `200`
  - `https://v3.elepcloud.com/v1/datasets`: `200`
  - `https://doc.elepcloud.com/`: `200`
- PostgreSQL:
  - database: `ai_data_platform_v3`
  - version: PostgreSQL `17.9`
  - `retrieval_evidences`: `2146` rows, `5712 kB`
  - `documents`: `2628` rows
  - `document_chunks`: `3745` rows
  - `retrieval_evidences` distinct datasets: `48`
  - `retrieval_evidences` distinct documents: `1035`
  - `retrieval_evidences` created range: `2026-05-18 22:48:53.064766+08` to `2026-06-07 21:18:24.493114+08`
- Lexical schema status:
  - `search_text/search_terms/search_tsv/search_language/indexed_content_hash/indexed_at`: not present
  - `retrieval_evidences_search_terms_gin_idx`: not present
  - `retrieval_evidences_search_tsv_gin_idx`: not present
  - `retrieval_evidences_scope_chunk_idx`: not present
  - `retrieval_evidences_indexed_content_hash_idx`: not present
- Current retrieval runtime env:
  - no `RETRIEVAL_SEARCH_BACKEND` configured
  - effective behavior remains default `legacy_scan`

Preflight interpretation:

- The table is small enough that the Phase 1 lexical migration is unlikely to
  be a long-running operation on 8, but it still includes a full-table backfill
  and normal index creation, so it should run only in an approved deploy step.
- Production lexical `EXPLAIN` cannot be recorded before the lexical schema is
  applied. The safe next deployment shape is schema/code with
  `RETRIEVAL_SEARCH_BACKEND=legacy_scan`, then EXPLAIN, then optional flag-on
  grey release.
- Live metrics need an explicit result collection path. The public dataset API
  is reachable, and 8 has NewBai-related datasets, but the public API does not
  expose a simple `retrieval.search` HTTP route. Metrics should be collected via
  assistant-run responses or a controlled JSONL generator.

## Remaining Gaps

- Real authenticated Recall@20, MRR@20, citation accuracy, and p95 latency are
  not yet recorded.
- Production `EXPLAIN` evidence for index usage is not recorded; only local
  Postgres index usage is covered.
- No GitHub sync or server deployment has been run from this smoke.
