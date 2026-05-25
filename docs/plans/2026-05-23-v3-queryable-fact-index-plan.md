# V3 Queryable Fact Index And Cross-Document Aggregation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make parsed V3 document evidence queryable and aggregatable at dataset scope so global search, cross-document statistics, table questions, and regression smokes use deterministic database-backed facts instead of retrieval top-k or ad hoc model counting.

**Architecture:** Keep third-party/public API fields, URLs, auth, and response contracts unchanged. Add internal fact-index tables and snapshot supply derived from existing `documents`, `document_chunks`, parse metadata, retrieval evidence, and current runtime scan logic. Run lightweight cleanup and fact extraction as an asynchronous post-ingest job after documents/chunks are stored, not as part of the synchronous parse path; AssistantRun continues to receive evidence through the existing `evidence_state.supplied_items` contract, but global/statistical prompts should prefer queryable facts and snapshot rows over raw chunk retrieval.

**Tech Stack:** Rust `storage`, `platform-api`, `ingest-worker`, `retrieval-worker`; PostgreSQL JSONB plus normalized internal tables; existing AssistantRun/ReAct evidence state; existing document quality smoke scripts; 8-server validation after review.

## Implementation Status

- Done: Tasks 1-4 internal schema, post-ingest cleanup queue, fact extraction/backfill, and bounded `entity_rows_by_type` snapshots.
- Done: Task 5 first pass. AssistantRun can supply `dataset_fact_snapshot`, compact it for model/retry input, convert supported snapshot rows into scan-compatible rows for existing direct answers, and expose only weak aggregate availability to ReAct planning.
- Guardrail: fact snapshot supply is skipped for external ACL filtered scopes or explicit selected-document scopes until scoped snapshots/query filtering exist; `ASSISTANT_RUN_FACT_INDEX_ENABLED=false` disables fact-backed supply.
- Guardrail: current snapshots only replace runtime scan for covered global entity prompts such as company/skill/project/keyword/year/section. Resume profile ranking,学历/学校, tables, and elevator point-list prompts continue to use the existing scan/detail paths.
- Done: Task 6 first pass. Document-quality smoke now records `answer_supply_sources` and `aggregate_answer_source`, supports local `-NoQualityGate`, rejects zero-test local smoke matches, and asserts no-gate aggregate sources for resume company stats, resume ranking, attendance, table/entity, and elevator point-list cases.
- Done: Task 7 first private 8-server smoke on 2026-05-23 against deployed HEAD `c5dbf78`: 7 configured private cases passed in no-gate mode. Observed aggregate sources were `dataset_entity_scan` for resume/smart-home/elevator and `spreadsheet_row_analysis` for attendance; `dataset_fact_snapshot` is expected only after this fact-index branch is deployed/backfilled.
- Status 2026-05-25: the fact-index implementation has since been deployed on the 8-server line. Current deployed HEAD is `c37d9638e`; a read-only 8-server database check after the latest scope work showed `document_facts=12321`, `document_fact_sources=12321`, and `dataset_fact_snapshots=7`.
- Next: rerun private smoke on the current 8-server HEAD expecting:
  - resume company statistics and other global entity-count questions prefer `dataset_fact_snapshot` when the scope is a full dataset/group;
  - selected-document, external ACL-filtered, or temporary conversation scopes keep using scoped document/detail or deterministic row analysis until scoped fact queries exist;
  - attendance remains `spreadsheet_row_analysis`;
  - resume ranking remains resume-profile deterministic rows;
  - smart-home/smart-elevator point-list questions may still use `dataset_entity_scan` until their fact snapshot kinds are explicitly promoted.

## Current Gaps - 2026-05-25

- Scoped fact aggregation is still missing for explicit selected-document scopes, third-party ACL-filtered scopes, and conversation temporary datasets.
- Fact snapshots are still snapshot-style supply, not an on-demand SQL aggregation API exposed to AssistantRun tools.
- There is no dedicated "why this aggregate source was chosen" debug summary beyond smoke fields and supplied item names.
- Post-ingest cleanup is designed to run after document入库, but the operator still needs an easy backfill/rerun command and visible per-dataset freshness.
- Table-heavy business facts, attendance row facts, and static-page metric facts are not all normalized into `document_facts`; domain-specific deterministic paths still carry much of the load.

---

## Design References

Use these as architectural references, not dependencies to vendor in directly:

- Docling: document parsing into a unified representation with tables, layout, reading order, OCR, and lossless JSON. This maps to V3's need for durable parse segments and table rows.
- Unstructured: partition documents into typed elements plus metadata before chunking/embedding. This maps to V3's need to separate "retrieval chunks" from "queryable parse elements."
- Microsoft GraphRAG: extracts structured data from unstructured text and uses graph memory structures for global reasoning. This maps to V3's cross-document/global questions.
- Graphiti: keeps provenance and temporal context for facts and supports queryable graphs. This maps to future "facts change over time" and source provenance.
- Haystack/LlamaIndex: useful references for explicit retrieval/indexing pipelines, but V3 should keep its own storage and visibility boundary.

## Core Decision

Do not try to solve cross-document statistics by making ReAct loop through documents. ReAct should decide when a global/cross-document query needs fact supply, but the actual count/list/rank result should come from internal queryable facts or aggregation snapshots.

Internal query priority for model-facing supply:

1. `dataset_fact_snapshots` or direct SQL aggregation over normalized facts.
2. Domain-specific deterministic analysis, e.g. `spreadsheet_row_analysis`.
3. `dataset_entity_scan` as compatibility fallback while facts backfill.
4. Retrieval evidence and `read_document_detail` for examples, source quotes, and validation only.

Post-ingest cleanup priority:

1. Parsing stores raw/structured evidence exactly as today.
2. Successful document入库 enqueues a lightweight cleanup/fact-index job.
3. The cleanup job normalizes deterministic evidence and writes internal facts/snapshots idempotently.
4. Cleanup failure should be retryable and observable, but must not overwrite raw parse evidence or change public API responses.
5. Expensive enhancement, e.g. VLM re-parse or model-based repair, remains conditional and budget-gated.

## Task 1: Add Internal Fact Schema

**Files:**

- Modify: `crates/storage/src/lib.rs`
- Test: `crates/storage/src/lib.rs`

**Step 1: Add failing storage schema assertions**

Extend `initial_schema_mentions_primary_tables` to assert the new internal tables exist:

```rust
assert!(INITIAL_SCHEMA.sql.contains("create table if not exists document_facts"));
assert!(INITIAL_SCHEMA.sql.contains("create table if not exists document_fact_sources"));
assert!(INITIAL_SCHEMA.sql.contains("create table if not exists dataset_fact_snapshots"));
```

**Step 2: Add tables to `INITIAL_SCHEMA`**

Add internal-only tables:

- `document_facts`
  - `id uuid primary key`
  - `tenant_id uuid not null`
  - `dataset_id uuid not null`
  - `document_id uuid not null`
  - `fact_type text not null`
  - `name text not null`
  - `normalized_name text not null`
  - `value_text text`
  - `value_number double precision`
  - `value_date date`
  - `attributes jsonb not null default '{}'::jsonb`
  - `confidence double precision not null default 1.0`
  - `source_kind text not null`
  - `source_locator text`
  - `source_chunk_id uuid`
  - `parse_version text`
  - `created_at timestamptz not null`

- `document_fact_sources`
  - fact-to-source detail rows when one fact is supported by multiple chunks/rows.

- `dataset_fact_snapshots`
  - `dataset_id`, `snapshot_kind`, `snapshot_key`, `snapshot_manifest`, `source_fact_count`, `source_document_count`, `created_at`
  - unique on `(tenant_id, dataset_id, snapshot_kind, snapshot_key)`.

Indexes:

- `(tenant_id, dataset_id, fact_type, normalized_name)`
- `(tenant_id, dataset_id, document_id, fact_type)`
- GIN on `attributes` and `snapshot_manifest`.

**Step 3: Add repository methods**

Add storage repository methods:

- `replace_document_facts(tenant_id, document_id, facts)`
- `list_document_facts_by_dataset(tenant_id, dataset_id, fact_type, limit)`
- `aggregate_document_facts_by_dataset(tenant_id, dataset_id, fact_type, limit)`
- `upsert_dataset_fact_snapshot(tenant_id, dataset_id, snapshot_kind, snapshot_key, manifest)`
- `get_dataset_fact_snapshot(tenant_id, dataset_id, snapshot_kind, snapshot_key)`

**Step 4: Verify**

Run:

```powershell
cargo test -p storage initial_schema_mentions_primary_tables --lib
```

Expected: schema and repository tests pass.

## Task 2: Queue Post-Ingest Cleanup And Fact Extraction

**Files:**

- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/ingest-worker/src/lib.rs`
- Modify: `crates/retrieval-worker/src/main.rs` if the existing worker is the better place to consume post-ingest indexing work.
- Test: `crates/ingest-worker/src/main.rs`

**Step 1: Add a post-ingest cleanup job contract**

After documents and chunks are successfully stored, enqueue a follow-up job for cleanup/fact indexing. The job payload should be internal only and include:

- `tenant_id`
- `dataset_id`
- `document_id`
- parse version or document updated timestamp
- retry count / reason when available

Do not change upload/parse public responses. The parse path may report success once the current document入库 work succeeds; cleanup status is tracked internally.

**Step 2: Convert stored evidence into fact candidates**

The cleanup job reads stored documents, chunks, and parse metadata, then derives facts from current metadata and text:

- organization/company
- person
- role/position
- skill/technology
- project/product/system
- location/area/floor/point
- school/degree/certificate
- year/date/time period
- section/table/paragraph signals

Keep current metadata extraction; add a parallel normalized fact output. Do not remove `candidate_terms` or existing metadata fields. The cleanup output is an index derived from raw evidence, not a replacement for raw evidence.

**Step 3: Add source provenance**

Each fact must carry:

- `document_id`
- `chunk_id` when available
- `chunk_index`
- `source_locator`
- `source_kind`: `chunk_understanding | document_structure | spreadsheet_row | parse_metadata | vlm_metadata`
- confidence and extraction method.

**Step 4: Replace facts idempotently**

At successful cleanup completion:

1. Re-read the document/chunk version to avoid writing facts for stale parses.
2. Delete/replace old facts for the document.
3. Insert new facts.
4. Mark cleanup/indexing status internally.

If fact insertion fails, retry or mark cleanup/indexing failed; do not mark the raw parse failed after it has already completed. AssistantRun must treat missing facts as unavailable and fall back to current runtime scans/retrieval.

**Step 5: Verify**

Run:

```powershell
cargo test -p ingest-worker document_structure --lib
cargo test -p ingest-worker chunk_understanding --lib
```

Expected: existing parsing tests pass, parse completion does not require fact indexing to finish synchronously, and new cleanup/fact extraction tests assert stable normalized names and source locators.

## Task 3: Backfill Facts From Existing Documents

**Files:**

- Create: `crates/platform-api/src/bin/fact-index-backfill.rs`
- Modify: `crates/platform-api/src/lib.rs` only for reusable helper extraction if needed.
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Add dry-run backfill CLI**

The CLI should accept:

```text
--dataset-id <uuid>
--document-id <uuid optional>
--limit <n>
--dry-run
```

It reads existing documents/chunks/metadata, runs the same cleanup/fact extraction path as the post-ingest job, and prints counts by fact type without writing when `--dry-run` is set.

**Step 2: Add write mode**

When not dry-run, call `replace_document_facts` per document and output:

- document count
- inserted fact count
- skipped document count
- parse-quality warnings

**Step 3: Verify**

Run local dry-run against fixture datasets if available:

```powershell
cargo run -p platform-api --bin fact-index-backfill -- --dataset-id <dataset-id> --dry-run
```

Expected: dry-run emits counts and writes nothing.

## Task 4: Build Dataset Aggregation Snapshots

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Add snapshot builder**

Implement internal snapshot kinds:

- `entity_counts_by_type`
- `entity_rows_by_type`
- `resume_profile_rows`
- `document_structure_rows`
- `table_signal_rows`
- `point_location_rows`

Each snapshot stores:

- `scanned_document_count`
- `source_fact_count`
- rows with `name`, `document_count`, `source_document_ids`, and representative locators.

**Step 2: Keep snapshots bounded**

Default row limits should match existing model limits. Store full internal counts, but model-facing supply should remain compact.

**Step 3: Verify**

Run:

```powershell
cargo test -p platform-api dataset_entity_scan --lib
cargo test -p platform-api assistant_run_general_entity_scan --lib
```

Expected: existing entity-scan behavior still passes, now with snapshot-backed rows where available.

## Task 5: Replace Runtime Scan Supply With Fact Supply

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/react_agent_catalog.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Add fact supply item**

Add a new internal supplied item shape:

```json
{
  "type": "dataset_fact_snapshot",
  "dataset_id": "...",
  "snapshot_kind": "entity_rows_by_type",
  "scanned_document_count": 10,
  "source_fact_count": 240,
  "entity_rows_by_type": {},
  "model_note": "Use this as the authoritative dataset-level aggregate for count/list/rank questions."
}
```

**Step 2: Preserve compatibility**

Keep `dataset_entity_scan` output for existing prompts and tests, but populate it from fact snapshots first. If no facts exist, fall back to the current runtime scan.

**Step 3: Update ReAct planning catalog**

When a prompt is global/cross-document/statistical, catalog should recommend:

- `retrieve_evidence` only for examples/source quotes.
- `read_document_detail` only for representative docs.
- `scan_dataset_entities` or future `query_dataset_facts` as the preferred aggregate action.

**Step 4: Verify**

Run:

```powershell
cargo test -p platform-api assistant_run_react_provider_input_uses_weak_planning_catalog --lib
cargo test -p platform-api assistant_run_general_entity_scan --lib
```

Expected: model-facing guidance says aggregate facts are authoritative.

## Task 6: Add Queryable Fact Regression Smokes

**Files:**

- Modify: `scripts/run-document-quality-smoke.ps1`
- Modify: `fixtures/document-quality/README.md`
- Test data: existing fixture datasets only; do not add large binaries without operator approval.

**Step 1: Add no-gate cross-document cases**

Run with `ASSISTANT_RUN_ANSWER_QUALITY_RETRY_BUDGET=0` and assert:

- resume company count uses fact snapshot/company rows;
- resume multi-sort uses `resume_profile_rows`;
- smart elevator point list is table-shaped from point/location facts;
- smart home feature list cites structured sections/keywords;
- table/attendance still uses deterministic row analysis.

**Step 2: Add source-of-answer checks**

Smoke should record whether answer came from:

- `dataset_fact_snapshot`
- `dataset_entity_scan`
- `spreadsheet_row_analysis`
- `retrieval_evidence`
- `read_document_detail`

Expected: global/cross-document stats should not pass if they only used retrieval top-k.

**Step 3: Verify**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/run-document-quality-smoke.ps1 -Mode Local
```

Expected: local fixture smokes pass or skip unavailable private cases cleanly.

## Task 7: 8-Server Validation

**Files:**

- No code changes beyond committed implementation.

**Step 1: Pre-deploy validation**

Run local focused tests:

```powershell
cargo test -p storage document_facts --lib
cargo test -p platform-api dataset_entity_scan --lib
cargo test -p platform-api assistant_run_general_entity_scan --lib
cargo test -p ingest-worker chunk_understanding --lib
```

**Step 2: Deploy only after operator approval**

Do not deploy until explicitly approved. Keep third-party public API contracts unchanged.

**Step 3: Server smoke**

Use private server case config and validate:

- no-gate mode;
- cross-document aggregates come from facts/snapshots;
- retrieval remains available for source examples;
- no quality gate customer-answer blocking is reintroduced.

## Rollback

Rollback should not require public API changes:

- Disable fact-backed supply with an env flag such as `ASSISTANT_RUN_FACT_INDEX_ENABLED=false`.
- Keep facts in DB but stop using them in AssistantRun supply.
- Fall back to current `dataset_entity_scan` and retrieval behavior.

## Success Criteria

- Cross-document count/list/rank questions are answered from database-backed facts or snapshots, not retrieval top-k.
- Smoke output records the aggregate source used.
- Resume company statistics, resume multi-sort, smart elevator point lists, and general entity statistics pass with answer quality gate disabled.
- Existing public third-party request/response fields, URLs, auth, and response contracts remain unchanged.
- ReAct remains a planner/orchestrator, not a substitute for deterministic global aggregation.
