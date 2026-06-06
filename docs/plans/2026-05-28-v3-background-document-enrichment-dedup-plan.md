# DataMax Background Document Enrichment And Dedup Implementation Plan

This plan is a living document. Update the checkboxes as work lands.

## Goal

Make document understanding a background capability of DataMax instead of a one-shot parse result:

- After a document enters the system, keep enriching its parsed content during idle time.
- Reuse parsed content for identical files across datasets and local/third-party uploads.
- Preserve existing external interfaces, URLs, auth, request fields, and response fields unless a later change is explicitly approved.
- Keep dataset authorization flexible: one parsed document can belong to multiple dataset scopes, and a conversation can authorize both dataset groups and individual documents.

## Current Findings

- `documents` currently stores a primary `dataset_id` and object location, but has no durable content fingerprint or canonical-document relationship.
- `dataset_document_memberships` already supports one document being visible in multiple dataset scopes.
- Dataset reads already union direct `documents.dataset_id` rows and membership rows through `list_by_dataset_scope`.
- Third-party parse currently downloads the file and creates a new `documents` row every time, even when content is duplicated.
- `ingest-worker` already queues post-ingest fact indexing through `workflow_tasks`.
- `retrieval-worker` already handles post-ingest fact cleanup and writes `document_facts`, `document_fact_sources`, and `dataset_fact_snapshots`.
- The elderly-care manual work showed that richer facts and section-aware extraction materially improve answers for procedures such as turning, medication checks, and nursing handover.

## Design Principles

- Do not block upload or chat on expensive enrichment.
- Do not delete duplicate local files in the first rollout.
- Deduplicate content only after tenant and permission checks.
- Keep external document IDs and uploaded document rows stable for traceability.
- Store reusable parsed artifacts once, then expose them through dataset membership or canonical read-through.
- Treat VLM reparse as a premium fallback for low parse quality, not a default path.

## Task 1: Add Fingerprint And Enrichment Schema

Status: `phase-1-schema-completed-locally-2026-06-06`

Files:

- `crates/storage/migrations/0013_document_canonical_enrichment.sql`
- `crates/storage/src/lib.rs`
- domain structs that mirror document metadata, if required

Changes:

- Add document fingerprint fields:
  - `content_sha256`
  - `content_size_bytes`
  - `canonical_document_id`
  - `dedup_state`
  - `deduped_at`
- Add `document_content_fingerprints`:
  - `tenant_id`
  - `content_sha256`
  - `content_size_bytes`
  - `canonical_document_id`
  - timestamps
  - primary key on `(tenant_id, content_sha256)`
- Add `document_enrichment_runs`:
  - `tenant_id`
  - `document_id`
  - `enrichment_kind`
  - `parse_version`
  - `input_fingerprint`
  - `status`
  - `priority`
  - `attempt_count`
  - `max_attempts`
  - run timestamps
  - `error_message`
  - `output_summary`
- Add idempotency index on `(tenant_id, document_id, enrichment_kind, input_fingerprint)`.

Acceptance:

- Migrations run locally and on 8 server.
- Existing documents remain readable.
- No public API contract changes.

Progress 2026-06-06:

- Added storage migration `0013_document_canonical_enrichment.sql`.
- Registered `DOCUMENT_CANONICAL_ENRICHMENT_SCHEMA` in storage migrations.
- Added table names for `document_content_fingerprints` and `document_enrichment_runs`.
- Added a schema regression for document fingerprint fields, canonical aliases, enrichment-run idempotency, and status/priority indexes.
- Verified locally with:
  - `cargo fmt --check -p storage`;
  - `cargo test -p storage document_canonical_enrichment --lib`;
  - `cargo test -p storage auth_migrations_are_registered_in_order --lib`;
  - `cargo check -p platform-api`.
- Not done yet:
  - applying the migration on 8 server;
  - backfilling fingerprints for existing documents;
  - canonical read-through in retrieval/facts.

## Task 2: Capture Content Fingerprints During Ingest

Status: `new-upload-capture-completed-backfill-tool-added-2026-06-06`

Files:

- `crates/platform-api/src/lib.rs`
- `crates/storage/src/lib.rs`
- local upload and third-party parse paths that create `NewDocument`

Changes:

- Compute SHA-256 and byte size while receiving or downloading file bytes.
- Store fingerprint fields on `documents`.
- Keep existing object storage behavior unchanged.
- Add a backfill command or maintenance script to compute fingerprints for existing local files.

Acceptance:

- Third-party parse and local upload both persist `content_sha256`.
- Existing idempotency behavior remains unchanged.
- Smoke upload of a large document still succeeds under the current 50 MB cap.

Progress 2026-06-06:

- Third-party external document parse now computes SHA-256 from downloaded bytes and records it with byte size after document creation.
- Main-site document registration now records SHA-256 and byte size when `object_key` resolves to a local file; unreadable or remote object keys are skipped without blocking registration.
- Zip archive expansion now records SHA-256 and byte size for extracted child documents before enqueueing child ingest workflows.
- Added `document-fingerprint-backfill` as a dry-run capable maintenance binary for existing documents. It supports `--dataset-id`, `--document-id`, `--limit`, `--dry-run`, `--include-existing`, `--summary-only`, and `--pretty`, and only records fingerprints for local object keys that resolve on the current server.
- Storage records the first seen content fingerprint as the canonical document and marks later matching content as duplicate without changing external document IDs.
- Added canonical read-through for duplicate document reads:
  - document chunks can be read through `list_by_document_or_canonical`;
  - retrieval evidence can be read through `list_by_document_or_canonical`;
  - dataset-level and document-scoped fact aggregates resolve duplicate documents to canonical document facts;
  - dataset retrieval evidence includes canonical evidence for documents assigned to the dataset.
- Added regression coverage in `external_document_parse_endpoint_downloads_and_enqueues_ingest` for:
  - `documents.content_sha256`;
  - `documents.content_size_bytes`;
  - `documents.canonical_document_id`;
  - `documents.dedup_state`;
  - `document_content_fingerprints.canonical_document_id`.
- Added regression coverage in `register_document_records_local_content_fingerprint` for:
  - local object-key fingerprint capture through `/v1/documents`;
  - canonical document alias creation in `document_content_fingerprints`.
- Added regression coverage in `canonical_duplicate_read_through_reuses_chunks_evidence_and_facts` for:
  - duplicate-document chunk read-through;
  - duplicate-document retrieval evidence read-through;
  - duplicate-dataset retrieval evidence read-through;
  - duplicate-dataset and duplicate-document fact aggregate read-through.
- Verified locally with:
  - `cargo fmt --check -p platform-api -p storage`;
  - `cargo test -p platform-api --bin document-fingerprint-backfill`;
  - `cargo test -p platform-api canonical_duplicate_read_through_reuses_chunks_evidence_and_facts --lib`;
  - `cargo test -p platform-api register_document_records_local_content_fingerprint --lib`;
  - `cargo test -p platform-api create_zip_document_ingest_records_child_content_fingerprint --lib`;
  - `cargo test -p platform-api external_document_parse_endpoint_downloads_and_enqueues_ingest --lib`;
  - `cargo test -p platform-api external_document_parse --lib`;
  - `cargo test -p storage document_canonical_enrichment --lib`;
  - `cargo check -p platform-api`.
- Not done yet:
  - 8-server migration rollout;
  - existing-document fingerprint backfill dry-run/execution on 8 server;
  - 8-server live duplicate read-through smoke.

## Task 3: Canonical Dedup Without Breaking Document IDs

Status: `completed-locally-new-document-aliases-2026-06-06`

Files:

- `crates/platform-api/src/lib.rs`
- `crates/storage/src/lib.rs`
- optional helper module such as `document_dedup.rs`

Changes:

- On new document creation, look up `(tenant_id, content_sha256)`.
- If no match exists:
  - mark the new document as canonical
  - insert `document_content_fingerprints`
- If a match exists:
  - keep the new document row for external traceability
  - set `canonical_document_id`
  - mark `dedup_state = duplicate`
  - do not immediately delete the uploaded object
- Add dataset membership for the canonical document where appropriate, so one parsed body can serve multiple dataset scopes.

Acceptance:

- A duplicate upload keeps its original document ID and external document ID.
- A dataset authorized through the duplicate can still answer from canonical parsed content.
- No duplicate facts are counted twice in dataset-level aggregation.

Progress 2026-06-06:

- `record_content_fingerprint` records the first seen content fingerprint as canonical and marks later matching documents as `dedup_state=duplicate`.
- New document IDs and third-party external document IDs remain stable; no object deletion is performed.
- Dataset authorization through duplicate documents is handled by canonical read-through instead of mutating public document identity.
- Verified by `canonical_duplicate_read_through_reuses_chunks_evidence_and_facts`.

Remaining:

- 8-server migration/backfill rollout and live duplicate smoke.

## Task 4: Canonical Read-Through In Retrieval And Facts

Status: `completed-locally-2026-06-06`

Files:

- `crates/storage/src/lib.rs`
- `crates/platform-api/src/lib.rs`
- `crates/retrieval-worker/src/main.rs`
- `crates/platform-api/src/fact_index.rs`

Changes:

- Resolve authorized document IDs to canonical content IDs only after authorization.
- Let duplicate aliases read chunks, facts, and snapshots from their canonical document when needed.
- Skip full re-indexing for duplicate aliases and record metadata such as `retrieval_index: reused_canonical`.
- Ensure global and cross-document statistics collapse duplicate aliases unless the user explicitly requests upload-level records.

Acceptance:

- Dataset-scope search finds facts through canonical content.
- Individual duplicate document authorization still works.
- Resume/project-experience style queries across many documents do not lose documents due to dedup.

Progress 2026-06-06:

- Added storage read-through methods for document chunks and retrieval evidence.
- Dataset retrieval evidence and dataset/document fact aggregations resolve duplicate document aliases to canonical content.
- Platform document detail/media/detail routes and ReAct `read_document_detail` use read-through methods after authorization.
- Verified by `canonical_duplicate_read_through_reuses_chunks_evidence_and_facts`.

Remaining:

- Skipping full duplicate re-indexing is not enabled yet; current rollout preserves write paths and proves read-through first.
- 8-server live duplicate read-through smoke remains required.

## Task 5: Background Enrichment Orchestrator

Status: `worker-execution-loop-completed-locally-2026-06-06`

Files:

- `crates/ingest-worker/src/main.rs`
- `crates/retrieval-worker/src/main.rs`
- `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`
- `crates/platform-api/src/fact_index.rs`
- optional new module `document_enrichment.rs`

Changes:

- After parse/index completes, enqueue asynchronous enrichment tasks:
  - `structure_outline_v1`
  - `fact_index_v2`
  - `qa_seed_v1`
  - `entity_relation_v1`
- Make each task idempotent by:
  - `tenant_id`
  - `document_id`
  - `enrichment_kind`
  - `content_sha256`
  - `parse_version`
- Add feature flags:
  - `DOCUMENT_ENRICHMENT_ENABLED`
  - `DOCUMENT_ENRICHMENT_KINDS`
  - `DOCUMENT_ENRICHMENT_IDLE_ONLY`
  - `DOCUMENT_ENRICHMENT_MAX_CONCURRENCY`
  - `DOCUMENT_ENRICHMENT_PREMIUM_VLM_ENABLED`
- Prefer idle execution or low-priority queue processing so chat latency is not affected.
- Use VLM/deep reparse only when parse quality is poor or operator policy allows it.

Acceptance:

- Upload returns before enrichment finishes.
- Failed enrichment retries with backoff and is visible in diagnostics.
- Disabling the feature flags returns the system to current behavior.

Progress 2026-06-06:

- Added storage structs and repository for `document_enrichment_runs`.
- Repository supports idempotent create/get, claim next available run, success marking, transient-error requeue with backoff time, terminal failure, and list-by-document.
- Retrieval worker post-ingest fact cleanup can enqueue `structure_outline_v1`, `fact_index_v2`, `qa_seed_v1`, and `entity_relation_v1` runs when `DOCUMENT_ENRICHMENT_ENABLED=true`.
- Enqueue is skipped by default, and skipped when a document has no `content_sha256`.
- Enqueue can now be narrowed with `DOCUMENT_ENRICHMENT_KINDS`, a comma-separated list that accepts versioned names and plan aliases such as `fact_index_v2`, `table_structure`, `procedure_steps`, `resume_profile`, and `spreadsheet_metrics`. Unknown entries are ignored; if a provided list has no valid kinds, post-ingest enqueue records `no_enabled_enrichment_kinds`.
- Document metadata records a compact `document_enrichment` enqueue summary after fact cleanup.
- Added read-only diagnostics endpoint `GET /v1/documents/{document_id}/enrichment-runs`, guarded by the existing document visibility check.
- Added a low-priority standalone `document-enrichment-worker` binary under `retrieval-worker`.
- The worker explicitly claims `document_enrichment_runs` and is not started by the normal retrieval worker process unless operations configures it.
- The worker supports:
  - `structure_outline_v1`: deterministic section/heading outline from chunk metadata and content headings;
  - `fact_index_v2`: deterministic fact rebuild and dataset entity snapshot refresh, with duplicate alias runs skipped through canonical read-through;
  - `qa_seed_v1`: compact likely-question seeds derived from document title and sections;
  - `entity_relation_v1`: deterministic entity/fact samples and relation hints from local fact candidates.
- Operational knobs:
  - `DOCUMENT_ENRICHMENT_WORKER_KIND`;
  - `DOCUMENT_ENRICHMENT_WORKER_POLL_INTERVAL_MS`;
  - `DOCUMENT_ENRICHMENT_WORKER_ERROR_BACKOFF_SECONDS`;
  - `DOCUMENT_ENRICHMENT_WORKER_ONCE`;
  - `DOCUMENT_ENRICHMENT_WORKER_MAX_RUNS`;
  - `DOCUMENT_ENRICHMENT_WORKER_DATABASE_MAX_CONNECTIONS`.
- Added regression `document_enrichment_run_repository_claims_requeues_and_succeeds`.
- Added regression `list_document_enrichment_runs_returns_visible_document_runs`.
- Added deterministic worker regressions for structure outline extraction, QA seed generation, and entity/fact summary generation.
- Verified locally with:
  - `cargo fmt --check -p platform-api -p storage`;
  - `cargo fmt --check -p retrieval-worker`;
  - `cargo test -p platform-api list_document_enrichment_runs_returns_visible_document_runs --lib`;
  - `cargo test -p platform-api document_enrichment_run_repository_claims_requeues_and_succeeds --lib`;
  - `cargo test -p platform-api canonical_duplicate_read_through_reuses_chunks_evidence_and_facts --lib`;
  - `cargo test -p retrieval-worker --bin document-enrichment-worker`;
  - `cargo test -p retrieval-worker`;
  - `cargo test -p storage document_canonical_enrichment --lib`;
  - `cargo check -p retrieval-worker --bin document-enrichment-worker`;
  - `cargo check -p retrieval-worker`;
  - `cargo check -p platform-api`.

Remaining:

- Wire feature flags before enabling on 8 server.
- Build and install the new worker binary on 8 server only after migration/backfill smoke.
- Run one-shot 8-server smoke with `DOCUMENT_ENRICHMENT_WORKER_ONCE=true` before enabling continuous polling.

## Task 6: Enrichment Outputs For Dense Manuals

Status: `phase-2-local-smoke-completed-2026-06-06`

Target documents:

- Elderly-care operating manuals
- Smart home manuals
- Smart elevator-control manuals
- Attendance/work-hour spreadsheets
- Resume collections

Extract:

- Clean heading hierarchy and section boundaries.
- Table/list blocks with semantic labels.
- Procedure steps.
- Checklists.
- Time intervals and thresholds.
- Entity aliases and relations.
- Aggregation-friendly facts.
- Likely QA seeds for common customer questions.

Examples:

- Medication distribution checks:
  - bed number
  - name
  - medicine name
  - concentration
  - dose
  - time
  - method
  - validity period
  - medical order
- Nursing handover:
  - medication status
  - physical abnormality
  - emotional abnormality
  - bed condition
  - bowel and urine condition
  - pressure injury or skin condition
  - infusion
  - oxygen
  - tubes
  - belongings
  - records
- Turning and pressure-injury prevention:
  - bedridden residents turn at least once every 2 hours
  - wheelchair residents change position at least once every 0.5 hours
  - avoid dragging, pulling, and pushing during movement

Acceptance:

- Demo questions cite enriched evidence instead of generic fallback knowledge.
- The three elderly-care smoke questions pass:
  - 长期卧床老人多长时间翻身一次？
  - 给老人发药时，需要执行哪些核对步骤？
  - 护理交接班时，必须交接的内容有哪些？

Progress 2026-06-06:

- Added post-ingest enqueue coverage for:
  - `table_structure_v1`;
  - `entity_terms_v1`;
  - `procedure_steps_v1`;
  - `resume_profile_v1`;
  - `spreadsheet_metrics_v1`.
- Added worker support and aliases for plan-level names:
  - `section_outline`;
  - `table_structure`;
  - `entity_terms`;
  - `procedure_steps`;
  - `resume_profile`;
  - `spreadsheet_metrics`.
- Added deterministic `output_summary` builders for:
  - table structure extraction from metadata and Markdown tables;
  - procedure step and threshold extraction for elderly-care manuals;
  - entity term rows grouped by fact type;
  - resume profile dimensions including candidate name, organizations, projects, skills, years, cities, and certificates;
  - spreadsheet/attendance metrics including absence rows and longest/shortest work-hour rows.
- Extended `fact_index` with `procedure_step` and `time_threshold` fact types so procedure/threshold facts can enter dataset entity snapshots.
- Added fixture coverage in `fixtures/document-quality/smoke-cases.json` for elderly-care procedure facts and enrichment-specific resume/table/attendance checks.
- Verified locally with:
  - `cargo fmt --check -p platform-api -p retrieval-worker`;
  - `cargo test -p platform-api fact_index --lib`;
  - `cargo test -p retrieval-worker`;
  - `cargo check -p retrieval-worker`;
  - `cargo check -p platform-api`;
  - `powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local`.
- Full local document-quality smoke passed and wrote:
  - JSON: `target/document-quality-smoke/document-quality-smoke-20260606T005235Z-6760.json`;
  - Markdown: `target/document-quality-smoke/document-quality-smoke-20260606T005235Z-6760.md`.

Remaining:

- Add broader spreadsheet table metric extraction once real workbook row shapes are confirmed.
- Run 8-server enrichment one-shot/backfill smoke after deployment.

## Task 7: Existing 8 Server Backfill And Dedup

Status: `guarded-dry-run-refreshed-2026-06-06`

Steps:

- Deploy schema and fingerprint capture first.
- Run fingerprint backfill in dry-run mode.
- Produce a duplicate candidate report:
  - content hash
  - document IDs
  - dataset IDs
  - titles
  - object keys
  - parse/index status
- Link canonical documents for exact content duplicates.
- Add dataset memberships for reused canonical content.
- Run enrichment backfill for priority documents.
- Run fact-index backfill only through `--dry-run --summary-only` first. Any real fact-index run must use `--confirm-real-run`; dataset-level real runs must use an explicit `--limit <= 5`.
- Run enrichment enqueue only through `document-enrichment-backfill --dry-run --summary-only` first. Any real dataset-level enrichment enqueue must use `--confirm-real-run`, exactly one `--kind`, an explicit `--limit <= 5`, and a reviewed scope with `missing_fingerprint_count=0`.
- Do not delete duplicate files in the first pass.

Acceptance:

- Existing third-party and local documents remain reachable.
- Duplicate exact-content documents no longer require duplicate parsing/fact extraction.
- Existing DataMax answers improve without requiring users to re-upload files.

Progress 2026-06-06:

- Re-ran the reviewed blocked dataset dry-runs on 8 server at current head:
  - receipt directory: `/srv/aiv3/repo/target/historical-enrichment-task5-dryrun-20260606T141645Z`;
  - fingerprint dry-run: `candidate_count=20`, `recorded_count=0`, `would_record_count=0`, `skipped_count=20`, `skipped_reason_counts.file_not_found=20`;
  - fact-index dry-run: `document_count=5`, `derived_fact_count=139`, `inserted_fact_count=0`, `snapshot_updated=false`;
  - enrichment precheck: `document_count=10`, `missing_fingerprint_count=10`, `would_enqueue_count=0`, `enqueued_count=0`.
- Re-ran the reachable single-document enrichment precheck:
  - receipt directory: `/srv/aiv3/repo/target/historical-enrichment-task5-ready-doc-precheck-20260606T141749Z`;
  - dataset `1bcf2529-0bbb-46e6-884f-c2b33db352c2`;
  - document `00fc651b-99f7-444b-9ee7-59695b2736cf`;
  - `missing_fingerprint_count=0`, `would_enqueue_count=2`, `enqueued_count=0`;
  - `procedure_steps_v1=1`, `table_structure_v1=1`.
- No real historical fingerprint, fact-index, or enrichment records were written.
- Next step still requires explicit operator approval before running a real single-document enqueue.

## Task 8: Observability

Status: `pending`

Files:

- internal task/status endpoints
- existing observability pages
- parse detail diagnostics

Show:

- content hash prefix
- canonical document ID
- dedup state
- duplicate count
- dataset memberships
- enrichment task status
- last run and next eligible run
- last error summary

Acceptance:

- Operators can tell whether a document is parsed, indexed, enriched, deduped, or stuck.
- Public third-party docs remain stable unless optional diagnostics are approved later.

## Task 9: Rollout And Rollback

Status: `pending`

Rollout order:

1. Fingerprint capture only.
2. Enrichment background tasks.
3. Enrichment backfill for priority documents.
4. Canonical read-through for duplicate aliases.
5. Dataset membership reuse.
6. Optional storage cleanup after separate approval and backup-first deletion.

Rollback:

- Disable enrichment flags.
- Disable canonical read-through.
- Keep alias document rows intact.
- Keep original files intact.
- No destructive cleanup in this plan.

## Smoke Suite

Run before deployment:

- Third-party plain QA.
- Third-party `dataset_external_ids` authorization reuse.
- Individual `available_document_external_ids` authorization reuse.
- Dataset plus standalone document authorization.
- Duplicate upload exact-content smoke.
- Elderly-care three-question smoke.
- Resume project-experience cross-document smoke.
- Attendance table date and work-hour query smoke.
- Static page fast HTML artifact smoke.

Run after deployment on 8 server:

- Health check for `platform-api`.
- Worker status check for `ingest-worker` and `retrieval-worker`.
- One local upload and one third-party parse.
- One enrichment backfill dry run.
- One duplicate candidate report.

## Open Decisions

- Whether duplicate aliases should appear as separate records in document lists by default, or collapse visually with an upload/history count.
- Whether optional third-party parse detail should expose dedup/enrichment diagnostics later.
- Whether `entity_relation_v1` should remain relational tables first, or later graduate to a time-aware graph store.
- What threshold should trigger VLM reparse:
  - low text coverage
  - image-heavy PDF
  - bad table confidence
  - repeated customer dissatisfaction
  - manual operator request

## Recommended Next Step

Start with Task 1 and Task 2 together:

- Add schema and storage support.
- Capture fingerprints for all new uploads.
- Add a dry-run backfill report for existing documents.

This gives immediate observability and prepares dedup/enrichment without changing customer behavior.
