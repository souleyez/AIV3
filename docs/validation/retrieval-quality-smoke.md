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

For deployment-target probes that intentionally use a small live sample instead
of the full 30+ case baseline matrix, pass `--live-subset`. The default baseline
coverage rules remain unchanged when this flag is omitted.

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

## 2026-06-09 8 Server Schema-Off Deployment

- Scope: GitHub main deployment to 8 server with lexical schema/code present and
  retrieval feature flag still off.
- GitHub commit: `5fdb27b78d71348214a3773c8944ca80a1b6851f`
- Server update:
  - `/srv/aiv3/repo` fast-forwarded from `6939a43ab` to `5fdb27b78`
  - release build: `CC=clang CXX=clang++ cargo build --release -p platform-api -p retrieval-worker`
  - build duration: `4m 13s`
  - restarted only:
    - `aiv3-platform-api.service`
    - `aiv3-retrieval-worker.service`
- Runtime flag status:
  - `/etc/aiv3/aiv3.env`: no `RETRIEVAL_SEARCH_BACKEND`
  - platform process env: no `RETRIEVAL_SEARCH_BACKEND`
  - effective retrieval backend remains default `legacy_scan`
- Service status after restart:
  - `aiv3-platform-api.service`: active
  - `aiv3-retrieval-worker.service`: active
  - `http://127.0.0.1:3000/healthz`: ok
  - `http://127.0.0.1:3000/readyz`: ready
  - `https://v3.elepcloud.com/`: `200`
  - `https://v3.elepcloud.com/v1/datasets`: `200`
  - `https://doc.elepcloud.com/`: `200`
- Logs:
  - no warning-or-higher entries for `aiv3-platform-api.service` in the checked
    deployment window
  - no warning-or-higher entries for `aiv3-retrieval-worker.service` in the
    checked deployment window

Schema verification after restart:

- `retrieval_evidences` lexical columns present:
  - `search_text`
  - `search_terms`
  - `search_tsv`
  - `search_language`
  - `indexed_content_hash`
  - `indexed_at`
- Lexical indexes present:
  - `retrieval_evidences_indexed_content_hash_idx`
  - `retrieval_evidences_scope_chunk_idx`
  - `retrieval_evidences_search_terms_gin_idx`
  - `retrieval_evidences_search_tsv_gin_idx`
- Backfill coverage:
  - total retrieval evidences: `2146`
  - `search_text`: `2146`
  - `search_tsv`: `2146`
  - `indexed_content_hash`: `2146`
  - `indexed_at`: `2146`
- `retrieval_evidences` size after migration: `15 MB`

Production EXPLAIN notes:

- Dataset used: `xinbai-project-materials`
  (`d4923d83-6053-4feb-8005-b22ee51e0227`)
- Full lexical-style query for `取高机会`, normal planner:
  - execution time: `19.423 ms`
  - plan used `retrieval_evidences_scope_chunk_idx`
  - planner did not choose GIN term/tsv indexes for the full query because the
    table is small and the scoped tenant scan is cheap
- GIN index proof:
  - `search_terms ?| ...` with `enable_seqscan=off` used
    `retrieval_evidences_search_terms_gin_idx`
  - `search_tsv @@ plainto_tsquery('simple', 'retrieval')` with
    `enable_seqscan=off` used `retrieval_evidences_search_tsv_gin_idx`
- Important limitation:
  - existing pre-migration rows have lexical columns backfilled, but historical
    manifests often do not contain `lexical.search_terms`, so Chinese business
    term recall on old evidence can still rely on `search_text` phrase matching
    and scoped scanning
  - new retrieval-worker output writes `lexical.search_text`,
    `lexical.search_terms`, and `indexed_content_hash`, so new/reindexed
    evidence will have stronger term-index coverage

8 server fixture smoke:

- Command: `RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true bash scripts/run-retrieval-quality-smoke.sh --baseline`
- Result: passed
- JSON report: `/srv/aiv3/repo/target/retrieval-quality-smoke/retrieval-quality-smoke-20260608T232252Z.json`
- Fixture count: `35`
- Permission leak count: `0`
- Metrics recorded: `false`

Deployment interpretation:

- Schema/code deployment with feature flag off is complete.
- Mainline runtime remains on `legacy_scan`.
- Production EXPLAIN evidence is sufficient to prove new indexes exist and are
  usable, but not sufficient to enable `postgres_lexical` by default.
- Remaining gates before flag-on: fix/rerun the weak live ranking cases and add
  assistant-run/customer-answer quality evidence.

## 2026-06-09 8 Server NewBai Live Retrieval Metrics

- Scope: controlled server-local retrieval-level live subset; no service
  restart, no feature flag change, no assistant-run/LLM answer generation.
- Server repo commit: `ec8856df0`
- Dataset: `xinbai-project-materials`
  (`d4923d83-6053-4feb-8005-b22ee51e0227`)
- Generator: sequential `target/release/retrieval-search-cli search ... --limit
  20` calls, converted to JSONL for the smoke evaluator.
- Smoke command:

```bash
RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true bash scripts/run-retrieval-quality-smoke.sh \
  --baseline \
  --live-subset \
  --cases target/retrieval-quality-smoke/live-8-newbai-20260608T233723Z/live-8-newbai-cases.jsonl \
  --results-jsonl target/retrieval-quality-smoke/live-8-newbai-20260608T233723Z/live-8-newbai-results.jsonl \
  --require-metrics \
  --report-dir target/retrieval-quality-smoke/live-8-newbai-20260608T233723Z/reports
```

- Result: passed
- JSON report:
  `target/retrieval-quality-smoke/live-8-newbai-20260608T233723Z/reports/retrieval-quality-smoke-20260608T233724Z.json`
- Fixture policy: `live_subset`
- Case count: `8`
- Recall@20: `1.0`
- MRR@20: `0.594246`
- Citation accuracy: `1.0`
- Answer pattern match rate: `0.625`
- p95 latency ms: `88.0`
- Permission leak count: `0`

Expected-source rank observations:

- `live-8-newbai-001`: expected source rank `18`; top hit was same document but
  not the expected sheet-summary chunk.
- `live-8-newbai-002`: expected source rank `1`.
- `live-8-newbai-003`: expected source rank `2`; top hit was the January low
  activity sheet while the prompt asked for February.
- `live-8-newbai-004`: expected source rank `1`.
- `live-8-newbai-005`: expected source rank `1`.
- `live-8-newbai-006`: expected source rank `1`.
- `live-8-newbai-007`: expected source rank `18`; top hit was the same workbook
  but a row chunk rather than the workbook summary chunk.
- `live-8-newbai-008`: expected source rank `7`; top hit was the related fixed
  vs commission workbook rather than the expected `quekou/xuzengxiaoshou`
  implementation chunks.

Interpretation:

- The live subset proves the current deployed retrieval path can produce
  structured live metrics and has no permission leak in this sample.
- It does not prove full assistant-run answer quality; generated `answer` values
  are retrieval-result summaries, not model completions.
- The low MRR and rank-18 cases show that source ranking still needs work before
  enabling `RETRIEVAL_SEARCH_BACKEND=postgres_lexical` by default.
- Because the dataset uses historical evidence with empty `search_terms`, this
  live subset should be re-run after reindexing or after a ranking change.

## 2026-06-09 Local NewBai Ranking Fix Receipt

- Scope: local code/test receipt before deployment; no `RETRIEVAL_SEARCH_BACKEND`
  change.
- Code path changed: `rank_retrieval_evidences_for_prompt` now adds bounded
  original-query signal scoring for retrieval evidence ranking.
- New ranking signals covered by tests:
  - Sheet summary / report overview wins when the prompt asks for calculation
    method or data-source style information.
  - Sheet summary / report overview wins when a table/report prompt asks for
    its content.
  - Exact numeric CJK literals such as `2月` and `4天` disambiguate same-family
    workbooks, with a small conflicting-month penalty.
  - ASCII business field names such as `quekou` and `xuzengxiaoshou` are strong
    exact-match anchors.

Local commands run:

```bash
cargo fmt --check -p platform-api
cargo test -p platform-api retrieval_ranking --lib
cargo test -p platform-api select_retrieval_evidence_ids_for_prompt --lib
cargo test -p platform-api retrieval_search_backend_parser_defaults_to_legacy_scan --lib
bash -n scripts/run-retrieval-quality-smoke.sh
git diff --check
RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true bash scripts/run-retrieval-quality-smoke.sh --baseline
bash scripts/run-retrieval-quality-smoke.sh --baseline
```

Local results:

- `retrieval_ranking`: `10` passed, including NewBai-style regression cases for
  calculation method, table content overview, month/day disambiguation, and
  ASCII field tokens.
- `select_retrieval_evidence_ids_for_prompt`: `4` passed.
- `retrieval_search_backend_parser_defaults_to_legacy_scan`: passed; default
  behavior remains `legacy_scan` unless the env flag is explicitly set.
- Full baseline smoke result: passed.
- Full baseline smoke JSON report:
  `target/retrieval-quality-smoke/retrieval-quality-smoke-20260609T002751Z.json`
- Fixture policy: `baseline`
- Fixture count: `35`
- Required fixture count: `30`
- Permission leak count: `0`
- Baseline metrics recorded: `false` as expected, because no live
  `--results-jsonl` was supplied.

Interpretation:

- This locally closes the specific P1-8 ranking bug class exposed by the 8
  server NewBai retrieval-level live subset.
- Assistant-run/customer-answer quality evidence is still separate.
- It does not justify enabling `postgres_lexical` by default.

## 2026-06-09 8 Server NewBai Ranking Rerun

- Scope: deploy GitHub main ranking fix to 8 server and rerun the same
  controlled NewBai retrieval-level live subset.
- GitHub/server commit: `141d04fc0`
- Deployed service: `aiv3-platform-api.service` only.
- Services not redeployed: `aiv3-retrieval-worker.service`, `aiv3-web.service`.
- Runtime flag status:
  - `/etc/aiv3/aiv3.env`: no `RETRIEVAL_SEARCH_BACKEND`
  - platform process env: no `RETRIEVAL_SEARCH_BACKEND`
  - effective retrieval backend remains default `legacy_scan`
- Health after restart:
  - `http://127.0.0.1:3000/healthz`: ok
  - `http://127.0.0.1:3000/readyz`: ready
  - `aiv3-platform-api.service`: active
  - `aiv3-retrieval-worker.service`: active
- Logs: no warning-or-higher entries for `aiv3-platform-api.service` in the
  checked post-deploy window.
- Live subset path:
  `target/retrieval-quality-smoke/live-8-newbai-20260609T003305Z`
- Smoke report:
  `target/retrieval-quality-smoke/live-8-newbai-20260609T003305Z/reports/retrieval-quality-smoke-20260609T003306Z.json`
- Result generator: sequential `target/release/retrieval-search-cli search ...
  --limit 20` calls with evidence excerpts from the returned retrieval evidence
  IDs. This remains retrieval-level evidence quality, not LLM answer quality.

Metrics:

- Fixture policy: `live_subset`
- Case count: `8`
- Recall@20: `1.0`
- MRR@20: `1.0`
- Citation accuracy: `1.0`
- Answer pattern match rate: `0.875`
- p95 latency ms: `95.0`
- Permission leak count: `0`

Expected-source ranks after the ranking fix:

- `live-8-newbai-001`: expected source rank `1` (was `18`)
- `live-8-newbai-002`: expected source rank `1`
- `live-8-newbai-003`: expected source rank `1` (was `2`)
- `live-8-newbai-004`: expected source rank `1`
- `live-8-newbai-005`: expected source rank `1`
- `live-8-newbai-006`: expected source rank `1`
- `live-8-newbai-007`: expected source rank `1` (was `18`)
- `live-8-newbai-008`: expected source rank `1` (was `7`)

Interpretation:

- P1-8 ranking weakness is closed for this controlled NewBai retrieval-level
  subset.
- The prior MRR@20 baseline was `0.594246`; the rerun is `1.0`.
- `postgres_lexical` remains disabled. This receipt does not authorize turning
  it on by default.
- Full assistant-run/customer-answer quality metrics are still required before
  treating customer-facing answer quality as closed.

## Remaining Gaps

- Full assistant-run/customer-answer live quality metrics are not yet recorded;
  the current live receipts are retrieval-level only.
- `RETRIEVAL_SEARCH_BACKEND=postgres_lexical` has not been enabled on 8 server.
