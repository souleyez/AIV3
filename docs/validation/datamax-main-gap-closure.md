# DataMax Main Gap Closure Validation

This ledger records evidence for `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`.

## Current 8-Server Baseline

- Date: 2026-06-06 00:12 +08:00.
- Host: `8服务器`.
- Repository path: `/srv/aiv3/repo`.
- Commit: `03458d310`.
- Git status: `## main...origin/main`; existing untracked `mode` observed on the server and not touched.
- Public endpoint: `https://v3.elepcloud.com`.
- Local platform port: `3000`.
- Platform API service: `aiv3-platform-api.service` active.
- Web service: `aiv3-web.service` active.
- Codex host agent service: `aiv3-codex-host-agent.service` active.
- Additional running DataMax services observed:
  - `aiv3-assistant-run-worker.service`;
  - `aiv3-chat-session-worker.service`;
  - `aiv3-codex-responses-shim.service`;
  - `aiv3-dataset-output-worker.service`;
  - `aiv3-external-action-worker.service`;
  - `aiv3-external-source-worker.service`;
  - `aiv3-ingest-worker.service`;
  - `aiv3-media-worker.service`;
  - `aiv3-memory-worker.service`;
  - `aiv3-report-planner-worker.service`;
  - `aiv3-report-render-worker.service`;
  - `aiv3-retrieval-worker.service`;
  - `aiv3-static-page-worker.service`.
- Model gateway status:
  - `GET http://127.0.0.1:3000/v1/model-gateway/status` returned HTTP `401`.
  - Body prefix: `{"code":"auth_session_required","message":"请先登录主系统后再管理模型池"}`.
  - This confirms the status surface is operator-session protected; production validation still needs a private operator session/cookie or a sanitized operator-side receipt.
- Model gateway profile database snapshot:
  - `rightcode-gpt-5-5-default | assistant_chat | rightcode | gpt-5.5 | enabled=true | max_concurrency=20 | rpm_limit=120 | timeout_ms=45000`.
  - `minimax-m2-7-fallback | assistant_chat | minimax | MiniMax-M2.7 | enabled=true | max_concurrency=6 | rpm_limit=120 | timeout_ms=120000`.
- Worker concurrency config:
  - Services load `/etc/aiv3/aiv3.env` and `/etc/aiv3/minimax.env`.
  - `aiv3-static-page-worker.service` also loads `/etc/aiv3/codex-orchestrator.env` and has drop-in `/etc/systemd/system/aiv3-static-page-worker.service.d/20-codex-orchestrator.conf`.
  - Raw env files were not printed because they may contain credentials.
  - `CHAT_SESSION_WORKER_CONCURRENCY=20`.
  - `STATIC_PAGE_IMAGE2_HTML_CONCURRENCY=5`.
  - `CODEX_HOST_CLOUDFLARE_CONCURRENCY=2`.
  - `EXTERNAL_CHANNEL_DIRECT_REPLY_ATTEMPT_TIMEOUT_MS=45000`.
  - `EXTERNAL_CHANNEL_DIRECT_REPLY_TOTAL_BUDGET_MS=120000`.
  - `LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE=true`.
  - `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED=true`.
- Database pool config:
  - `CHAT_SESSION_DATABASE_MAX_CONNECTIONS=10`.
  - `STATIC_PAGE_DATABASE_MAX_CONNECTIONS=8`.
  - `CODEX_HOST_DATABASE_MAX_CONNECTIONS=4`.
  - Platform API pool cap was not separately proven from the current snapshot.
- Workflow queue status:
  - `GET http://127.0.0.1:3000/v1/workflow-tasks/queue-stats` returned HTTP `200`.
  - Snapshot generated at `2026-06-05T16:12:48Z`.
  - `execution_count=157`, `task_count=372`.
  - Queue snapshot showed no `queued`, `running`, or `retrying` tasks in the visible queues.
  - Historical failed tasks remain in `external_source`, `ingest`, `static_page_image_preview`, and `static_page_render`; these are not current backlog but should stay visible to operators.

## Gate Results

| Gate | Status | Receipt |
| --- | --- | --- |
| P0 Gate A: 20-way concurrency | in progress | Read-only 8-server queue/status baseline recorded. 8-server third-party streaming smoke passed with active bearer: 10 ordinary, 3 static-page, 2 reconnect, 15/15 OK, duplicate final messages 0, P95 6015 ms. Full third-party 20-way, main-site 20-way, static-page 5-way, Cloudflare fallback 2-way, and report/document-quality private smoke remain pending. |
| P0 Gate B: report/static-page operations | in progress | Accepted-template reuse and Xinbai template contract validated locally/publicly on 2026-06-06. Production low-load prewarm is not enabled because `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED` is unset on 8 server. See `external-capability-routing-smoke.md` and `external-report-export-smoke.md` for latest full bearer-backed report smoke. |
| P1 Gate C: background enterprise memory | in progress | Storage schema phase 1 implemented for document fingerprints, canonical aliases, and enrichment runs. Third-party parse, main-site local register, and zip child-document creation now persist SHA-256/size and canonical fingerprint rows when bytes/files are available. A dry-run capable existing-document fingerprint backfill tool exists. Canonical read-through for chunks/evidence/facts, the enrichment-run repository foundation, feature-flagged post-ingest enrichment enqueue, document-level enrichment diagnostics, a standalone low-priority enrichment worker loop, Phase 2 deterministic enrichment kinds for tables/procedures/entities/resumes/spreadsheets, and local aggregate-first answer supply are implemented. Full local document-quality smoke and aggregate-first regressions passed. 8-server deploy to `1db91f69c3fd`, schema migration, build, service restart, dry-run fingerprint backfill, and one-shot worker startup are recorded. Production non-dry-run backfill/enrichment, live duplicate read-through smoke, and private aggregate smoke remain pending. |
| P1 Gate D: low-quality answer recovery | in progress | Passive local implementation and smoke passed on 2026-06-06. Hard gate remains disabled. Production enqueue remains configuration-gated by `CODEX_HOST_TASK_ENABLED` and `CODEX_HOST_TASK_ALLOWLIST`; 8-server live passive collection/enqueue smoke remains pending. |
| P1 Gate E: confirmed data ingestion | in progress | Local confirmed staging-to-dataset sync smoke passed on 2026-06-06. 8-server live source readiness passed for `hy-sql-traffic-area`. 8-server external data-ingestion analysis completed and returned a `v3_data_ingestion_staging_plan` with `human_review_required=true` and no raw credentials. Local operator-confirmation routing is now implemented and tested: configured model-gateway operators can confirm/sync external-channel staging plans, while anonymous and ordinary non-owner users still receive `assistant_run_not_found`. 8-server deploy plus authenticated operator confirm/sync smoke remain pending. |

## Rollout Receipts

### 2026-06-06 Baseline Receipt

- Commands run:
  - `ssh 8服务器 "cd /srv/aiv3/repo && git rev-parse --short HEAD && git status --short --branch && systemctl list-units --type=service --state=running 'aiv3*' --no-pager --plain"`
  - `ssh 8服务器 'for port in 3000 8080 8000; do ... /v1/model-gateway/status ...; done'`
  - `ssh 8服务器 'for port in 3000 8080 8000; do ... /v1/workflow-tasks/queue-stats ...; done'`
  - local private bearer environment presence check.
- Findings:
  - 8 server is running the expected DataMax service set.
  - Platform API responds on local port `3000`.
  - Model-gateway status is correctly operator-session protected.
  - Database profile snapshot confirms assistant-chat model pool target: Right `gpt-5.5` primary with concurrency 20 and MiniMax fallback with concurrency 6.
  - Runtime environment confirms main chat concurrency 20, static-page/Image2 concurrency 5, and Cloudflare fallback concurrency 2.
  - Workflow queue stats are reachable and report no current visible backlog.
  - Private 20-way mutation smoke cannot be completed from this local shell until a bearer/cookie is configured.

### 2026-06-06 No-Bearer Third-Party Probe

- Command:
  - `node scripts/smoke/external-channel-20way.mjs --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 2 --timeout-ms 15000 --output-dir target/external-channel-20way-smoke-no-bearer-probe`
- Receipt:
  - `target/external-channel-20way-smoke-no-bearer-probe/20260605161345.json`
- Result:
  - `okCount=0`, `failedCount=2`.
  - Both requests returned HTTP `401`.
  - Body prefix: `{"code":"external_channel_auth_failed","message":"external channel bearer token is missing or invalid"}`.
- Interpretation:
  - This is expected without a private bearer and confirms the external endpoint does not bypass third-party channel auth.
  - This probe is not a concurrency pass/fail result.

### 2026-06-06 No-Cookie Cloudflare Fallback Guard Probe

- Command:
  - `node scripts/smoke/cloudflare-fallback-2way.mjs --base-url https://v3.elepcloud.com --min-expected 0 --max-allowed 2 --output-dir target/cloudflare-fallback-2way-smoke-no-cookie-probe`
- Receipt:
  - `target/cloudflare-fallback-2way-smoke-no-cookie-probe/20260605161456.json`
- Result:
  - `ok=false`.
  - `workflowQueueStatsLoaded=true`.
  - `modelGatewayStatusLoaded=false`.
  - `codexWorkerPresent=false`.
  - `maxRunning=0`.
- Interpretation:
  - Queue status is publicly/read-only reachable through the existing diagnostics surface.
  - Codex host fallback concurrency cannot be proven without an operator-authenticated model-gateway status request.
  - This probe is not a Cloudflare fallback concurrency pass/fail result.

### 2026-06-06 Static-Page Template Reuse And Prewarm Audit

- Local tests:
  - `cargo test -p platform-api static_page_template_prewarm --lib` passed, 2 tests.
  - `cargo test -p platform-api static_page_template_match_tokens_allow_dataset_overlap --lib` passed, 1 test.
- Template validation:
  - `npm run validate:xinbai-report-template` passed.
  - `npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html` passed and checked 7 public files.
- 8-server runtime:
  - `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=UNSET`.
  - `STATIC_PAGE_TEMPLATE_PREWARM_DELAY_MINUTES=UNSET`.
- Interpretation:
  - Accepted-template matching/reuse and prewarm task construction are implemented and tested.
  - The accepted Xinbai report template remains valid locally and publicly.
  - Silent low-load template prewarm is not active in production until `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=true` is configured and a low-load execution smoke is recorded.

### 2026-06-06 Background Document Enrichment Schema Phase 1

- Files changed:
  - `crates/storage/migrations/0013_document_canonical_enrichment.sql`;
  - `crates/storage/src/lib.rs`.
- Schema additions:
  - document fingerprint fields on `documents`;
  - `canonical_document_id`, `dedup_state`, and `deduped_at` fields on `documents`;
  - `document_content_fingerprints`;
  - `document_enrichment_runs`;
  - idempotency index on `(tenant_id, document_id, enrichment_kind, input_fingerprint)`;
  - status/priority index for background enrichment scheduling.
- Local verification:
  - `cargo fmt --check -p storage` passed.
  - `cargo test -p storage document_canonical_enrichment --lib` passed.
  - `cargo test -p storage auth_migrations_are_registered_in_order --lib` passed.
  - `cargo check -p platform-api` passed.
- Remaining:
  - apply migration on 8 server during deployment;
  - backfill SHA-256/size for existing local documents;
  - add canonical read-through for retrieval/facts.

### 2026-06-06 Third-Party Parse Fingerprint Capture

- Files changed:
  - `crates/storage/src/lib.rs`;
  - `crates/platform-api/src/lib.rs`.
- Behavior:
  - external document parse computes SHA-256 from downloaded bytes;
  - `documents.content_sha256` and `documents.content_size_bytes` are updated after document creation;
  - first seen content is recorded in `document_content_fingerprints`;
  - current document is marked `dedup_state=canonical` with `canonical_document_id` pointing to itself;
  - later matching content can be marked duplicate by the same storage helper without changing external document IDs.
- Local verification:
  - `cargo fmt --check -p platform-api -p storage` passed.
  - `cargo test -p platform-api external_document_parse_endpoint_downloads_and_enqueues_ingest --lib` passed.
  - `cargo test -p platform-api external_document_parse --lib` passed.
  - `cargo check -p platform-api` passed.
- Remaining:
  - 8-server migration rollout and live upload/parse smoke;
  - existing-document fingerprint backfill dry-run/execution on 8 server;
  - 8-server live duplicate read-through smoke.

### 2026-06-06 Existing-Document Fingerprint Backfill Tool

- Files changed:
  - `crates/platform-api/src/bin/document-fingerprint-backfill.rs`.
- Behavior:
  - maintenance binary loads documents for the configured tenant;
  - supports `--dataset-id`, `--document-id`, `--limit`, `--dry-run`, `--include-existing`, and `--pretty`;
  - defaults to documents missing `content_sha256`;
  - computes SHA-256 and byte size only for object keys that resolve to a local file;
  - dry-run reports whether each document would record a canonical or duplicate fingerprint;
  - non-dry-run uses the same storage helper as new uploads, preserving document IDs.
- Example dry-run:
  - `cargo run -p platform-api --bin document-fingerprint-backfill -- --dataset-id <dataset_uuid> --limit 100 --dry-run --pretty`
- Local verification:
  - `cargo fmt --check -p platform-api -p storage` passed.
  - `cargo test -p platform-api --bin document-fingerprint-backfill` passed.
  - `cargo test -p platform-api register_document_records_local_content_fingerprint --lib` passed.
  - `cargo test -p platform-api create_zip_document_ingest_records_child_content_fingerprint --lib` passed.
  - `cargo test -p platform-api external_document_parse --lib` passed.
  - `cargo test -p storage document_canonical_enrichment --lib` passed.
  - `cargo check -p platform-api` passed.
- Remaining:
  - run 8-server migration before any live backfill;
  - run an 8-server dry-run and record counts before non-dry-run execution;
  - run 8-server live duplicate read-through smoke.

### 2026-06-06 Canonical Duplicate Read-Through

- Files changed:
  - `crates/storage/src/lib.rs`;
  - `crates/platform-api/src/lib.rs`;
  - `crates/platform-api/src/react_agent_tools.rs`.
- Behavior:
  - original write paths still use direct document IDs;
  - read paths can resolve duplicate document IDs to their canonical document for chunks and retrieval evidence;
  - dataset retrieval evidence includes canonical evidence for documents assigned to the requested dataset;
  - dataset-level and document-scoped fact aggregates resolve duplicate documents to canonical facts;
  - platform document detail, chunk/evidence routes, media detail, and ReAct `read_document_detail` use the read-through methods.
- Local verification:
  - `cargo fmt --check -p platform-api -p storage` passed.
  - `cargo test -p platform-api canonical_duplicate_read_through_reuses_chunks_evidence_and_facts --lib` passed.
  - `cargo test -p platform-api register_document_records_local_content_fingerprint --lib` passed.
  - `cargo test -p platform-api create_zip_document_ingest_records_child_content_fingerprint --lib` passed.
  - `cargo test -p platform-api external_document_parse --lib` passed.
  - `cargo test -p platform-api --bin document-fingerprint-backfill` passed.
  - `cargo test -p storage document_canonical_enrichment --lib` passed.
  - `cargo check -p platform-api` passed.
- Remaining:
  - 8-server migration/backfill rollout;
  - 8-server live duplicate read-through smoke.

### 2026-06-06 Document Enrichment Run Repository Foundation

- Files changed:
  - `crates/storage/src/lib.rs`;
  - `crates/platform-api/src/lib.rs`.
- Behavior:
  - storage exposes `document_enrichment_runs()`;
  - repository supports idempotent create/get by `(tenant_id, document_id, enrichment_kind, input_fingerprint)`;
  - repository can claim the next available pending run with `skip locked`;
  - transient errors can requeue with a future `available_at`;
  - terminal success/failure and list-by-document are available for diagnostics.
  - retrieval worker can enqueue `structure_outline_v1`, `fact_index_v2`, `qa_seed_v1`, and `entity_relation_v1` after post-ingest fact cleanup when `DOCUMENT_ENRICHMENT_ENABLED=true`;
  - enqueue is skipped by default and skipped for documents without `content_sha256`.
  - platform API exposes `GET /v1/documents/{document_id}/enrichment-runs` after existing document visibility checks.
- Local verification:
  - `cargo fmt --check -p platform-api -p storage` passed.
  - `cargo test -p platform-api list_document_enrichment_runs_returns_visible_document_runs --lib` passed.
  - `cargo test -p platform-api document_enrichment_run_repository_claims_requeues_and_succeeds --lib` passed.
  - `cargo test -p platform-api canonical_duplicate_read_through_reuses_chunks_evidence_and_facts --lib` passed.
  - `cargo test -p platform-api external_document_parse --lib` passed.
  - `cargo test -p platform-api --bin document-fingerprint-backfill` passed.
  - `cargo test -p retrieval-worker` passed.
  - `cargo test -p storage document_canonical_enrichment --lib` passed.
  - `cargo check -p retrieval-worker` passed.
  - `cargo check -p platform-api` passed.
- Remaining:
  - 8-server migration/backfill rollout and live smoke.

### 2026-06-06 Document Enrichment Worker Execution Loop

- Files changed:
  - `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`;
  - `docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md`;
  - `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`;
  - `docs/validation/datamax-main-gap-closure.md`.
- Behavior:
  - adds a standalone `document-enrichment-worker` binary under the existing `retrieval-worker` crate;
  - the binary is not part of the normal retrieval worker main process and only consumes enrichment runs when explicitly started by operations;
  - claims pending `document_enrichment_runs` by priority and `available_at`;
  - marks success with compact `output_summary`;
  - requeues transient errors with configurable backoff and lets the repository mark terminal failure after `max_attempts`;
  - supports one-shot execution through `DOCUMENT_ENRICHMENT_WORKER_ONCE=true` and bounded smoke through `DOCUMENT_ENRICHMENT_WORKER_MAX_RUNS`;
  - supports optional kind filtering through `DOCUMENT_ENRICHMENT_WORKER_KIND`.
- Enrichment kinds implemented in this local phase:
  - `structure_outline_v1`: extracts section/heading outline from chunk metadata and short content headings;
  - `fact_index_v2`: rebuilds deterministic document facts and dataset entity snapshots for canonical documents, while duplicate alias runs skip fact writes and report canonical read-through;
  - `qa_seed_v1`: creates compact likely-question seeds from document title and extracted sections;
  - `entity_relation_v1`: produces deterministic entity samples and relation hints from local fact candidates.
- Local verification:
  - `cargo fmt --check -p retrieval-worker` passed.
  - `cargo test -p retrieval-worker --bin document-enrichment-worker` passed, 3 tests.
  - `cargo check -p retrieval-worker --bin document-enrichment-worker` passed.
  - `cargo check -p retrieval-worker` passed.
- Remaining:
  - build the new binary on 8 server during the next controlled deployment;
  - run migration before any enrichment backfill;
  - run one-shot smoke with `DOCUMENT_ENRICHMENT_WORKER_ONCE=true`;
  - enable continuous polling only after queue/backoff behavior is confirmed.

### 2026-06-06 Background Fact Enrichment Phase 2 Local Coverage

- Files changed:
  - `crates/platform-api/src/fact_index.rs`;
  - `crates/retrieval-worker/src/main.rs`;
  - `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`;
  - `fixtures/document-quality/smoke-cases.json`;
  - `docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md`;
  - `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`;
  - `docs/validation/datamax-main-gap-closure.md`.
- Behavior:
  - post-ingest enrichment enqueue now includes `table_structure_v1`, `entity_terms_v1`, `procedure_steps_v1`, `resume_profile_v1`, and `spreadsheet_metrics_v1` when `DOCUMENT_ENRICHMENT_ENABLED=true`;
  - `document-enrichment-worker` supports versioned kinds and plan-name aliases for section outline, table structure, entity terms, procedure steps, resume profile, and spreadsheet metrics;
  - table summaries extract metadata/Markdown table shapes;
  - procedure summaries extract care-operation steps and time thresholds;
  - resume summaries extract candidate name, organizations, projects, skills, years, cities, and certificates;
  - spreadsheet summaries extract absence rows plus longest/shortest work-hour rows from normalized text rows;
  - `fact_index` emits `procedure_step` and `time_threshold` facts so care-operation rules can be aggregated in dataset fact snapshots.
- Fixture coverage:
  - added `elderly-care-procedure-facts`;
  - linked enrichment worker checks into resume ranking, table-heavy document, and frequent attendance cases.
- Local verification:
  - `cargo fmt --check -p platform-api -p retrieval-worker` passed.
  - `cargo test -p platform-api fact_index --lib` passed, 6 tests.
  - `cargo test -p retrieval-worker` passed.
  - `cargo check -p retrieval-worker` passed.
  - `cargo check -p platform-api` passed.
  - `powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local` passed.
- Smoke receipts:
  - Selected elderly-care smoke: `target/document-quality-smoke/document-quality-smoke-20260606T005136Z-21792.json`.
  - Selected resume/table/attendance smoke: `target/document-quality-smoke/document-quality-smoke-20260606T005218Z-22360.json`.
  - Full local document-quality smoke: `target/document-quality-smoke/document-quality-smoke-20260606T005235Z-6760.json`.
- Remaining:
  - run 8-server one-shot enrichment worker smoke after migration and deployment;
  - broaden spreadsheet extraction if the live workbook row shape differs from normalized text rows.

### 2026-06-06 Aggregate-First Answer Supply Local Coverage

- Files changed:
  - `crates/platform-api/src/lib.rs`;
  - `crates/platform-api/src/react_agent_catalog.rs`;
  - `docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md`;
  - `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`;
  - `docs/validation/document-understanding-smoke.md`;
  - `docs/validation/datamax-main-gap-closure.md`.
- Behavior:
  - added internal deterministic aggregate intent detection for document/entity aggregates, resume statistics/ranking, attendance row analysis, and Xinbai-style store/brand/take-high/risk/low-active metric prompts;
  - kept business metric aggregate detection separate from generic document entity scans, so database-backed operating questions can use `database_aggregate` without forcing a full document scan unless the prompt is explicitly document/file/table scoped;
  - tightened model-facing guidance for `dataset_fact_snapshot`, `document_facts_scoped_aggregate`, `database_aggregate`, `dataset_entity_scan`, and `spreadsheet_row_analysis` so totals and rankings cite deterministic scope/row counts and retrieval top-k remains secondary support;
  - updated ReAct planning catalog to advertise deterministic aggregate supply actions before retrieval top-k.
- Local verification:
  - `cargo fmt --check -p platform-api` passed.
  - `cargo test -p platform-api dataset_fact_snapshot --lib` passed, 2 tests.
  - `cargo test -p platform-api scoped_fact --lib` passed, 3 tests.
  - `cargo test -p platform-api database_aggregate_heuristics --lib` passed, 6 tests.
  - `cargo test -p platform-api assistant_run_general_entity_scan_prompts_request_dataset_scan --lib` passed, 1 test.
  - `cargo test -p platform-api assistant_run_deterministic_aggregate_intent_covers_customer_smoke_domains --lib` passed, 1 test.
  - `cargo test -p platform-api planning_catalog_prefers_aggregate_fact_supply_without_fact_rows --lib` passed, 1 test.
  - `cargo test -p platform-api assistant_run_answer_quality_judge_runs_for_structured_short_answer --lib` passed, 1 test.
  - `cargo test -p platform-api assistant_run_answer_quality_judge_skips_satisfied_spreadsheet_table --lib` passed, 1 test.
  - `cargo test -p platform-api assistant_run_answer_quality_gate_retries_deferred_retrieval_language --lib` passed, 1 test.
  - `cargo check -p platform-api` passed.
  - `powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local` passed.
- Smoke receipt:
  - JSON: `target/document-quality-smoke/document-quality-smoke-20260606T010841Z-27544.json`.
  - Markdown summary: `target/document-quality-smoke/document-quality-smoke-20260606T010841Z-27544.md`.
- Remaining:
  - run private 8-server aggregate smoke after deployment and private bearer/cookie configuration are available.

### 2026-06-06 Passive Answer Quality Autofix Local Coverage

- Files changed:
  - `crates/platform-api/src/lib.rs`;
  - `scripts/run-cloudflare-codex-fixed-task-smoke.ps1`;
  - `docs/operations/answer-quality-autofix.md`;
  - `docs/operations/cloudflare-codex-fixed-task-templates.md`;
  - `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`;
  - `docs/validation/datamax-main-gap-closure.md`.
- Behavior:
  - hard customer-facing answer gate remains disabled; collected packages record `blocking_gate_enabled=false`;
  - passive low-quality collection now records deterministic supply ignored by an insufficient-evidence answer, missing report/static-page artifact links, and repeated fallback/timeout/provider-failure events;
  - fixed-task smoke script accepts both separate `-Case` values and comma-separated case lists, matching the plan command;
  - operation docs now match the actual legacy smoke script path `scripts/run-v3-quality-gate-smoke.ps1`.
- Local verification:
  - `cargo fmt --check -p platform-api -p codex-host-agent` passed.
  - `cargo test -p platform-api answer_quality_autofix --lib` passed, 12 tests.
  - `cargo test -p codex-host-agent answer_quality --lib` passed, 3 tests.
  - `cargo test -p platform-api assistant_run_answer_quality_gate --lib` passed, 14 tests.
  - `powershell -ExecutionPolicy Bypass -File .\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary` passed.
  - `powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local` passed.
  - `cargo check -p platform-api -p codex-host-agent` passed.
- Smoke receipts:
  - Fixed-task JSON: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T011735Z.json`.
  - Fixed-task Markdown: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T011735Z.md`.
  - Quality-gate JSON: `target/document-quality-smoke/document-quality-smoke-20260606T011549Z-23164.json`.
  - Quality-gate Markdown: `target/document-quality-smoke/document-quality-smoke-20260606T011549Z-23164.md`.
- Remaining:
  - after deployment, run 8-server live passive collection/enqueue smoke with production-safe allowlist settings.

### 2026-06-06 Confirmed Data Ingestion Staging Sync Local Coverage

- Existing implementation verified:
  - internal confirm route: `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/confirm`;
  - internal sync route: `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/sync`;
  - confirmation consumes only an attached `v3_data_ingestion_staging_plan`;
  - confirmation creates or reuses a private DataMax staging dataset and records a single confirmation event;
  - sync refuses unconfirmed plans, validates the selected database source is included in the plan, starts `ExternalSourceSync`, and deduplicates repeated clicks by default;
  - workflow progress is recorded as sanitized AssistantRun sync update events;
  - completed staging dataset scope can be restored for later turns in the same external conversation.
- Local verification command:
  - `bash scripts/run-data-ingestion-staging-sync-smoke.sh`
- Passed checks:
  - `cargo test -p platform-api data_ingestion --lib` passed, 13 tests;
  - `cargo test -p platform-api external_source_sync --lib` passed, 6 tests;
  - `cargo test -p external-source-worker` passed, 11 tests;
  - `cargo test -p ingest-worker` passed;
  - `cargo test -p retrieval-worker` passed;
  - live database-source readiness report self-test passed through `DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true`;
  - `npm run check:pure-third-party-guide-html` passed.
- Smoke receipts:
  - JSON: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T012507Z.json`;
  - Markdown summary: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T012507Z.md`;
  - live readiness self-test JSON: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T012938Z.json`;
  - live readiness self-test Markdown: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T012938Z.md`.
- Safety contract:
  - third-party request fields are unchanged;
  - production writes, schema mutation, raw database credentials, and raw table dumps remain blocked;
  - sync requires a confirmed plan and an in-plan source id.
- Remaining:
  - deploy current code to 8 server;
  - run `bash scripts/run-data-ingestion-staging-live-smoke.sh` on 8 server against the stored database source;
  - manually confirm one reviewed staging plan and start sync through the internal operator routes;
  - record that source-derived documents, chunks, and retrieval evidence are available before customer-facing reporting from that staging dataset.

### 2026-06-06 Controlled Streaming Local Coverage

- Files changed:
  - `crates/platform-api/src/lib.rs`;
  - `docs/plans/2026-06-01-v3-streaming-session-upgrade-plan.md`;
  - `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`;
  - `docs/validation/datamax-main-gap-closure.md`.
- Behavior:
  - main-site `POST /v1/assistant-runs/{run_id}/continue/stream` now supports live answer deltas behind `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true`;
  - main-site new-run and continue-run stream paths share the same `AssistantRunLiveDeltaSink` worker/channel pattern;
  - continue stream completion skips the final full-text delta when live deltas were already emitted, preventing duplicate text in the browser;
  - JSON continue and background model-completion recovery remain non-streaming;
  - third-party streaming behavior and public fields were not changed in this slice.
- Local verification:
  - `cargo fmt --check -p platform-api` passed;
  - `cargo test -p platform-api assistant_run_sse --lib` passed, 5 tests;
  - `cargo test -p platform-api assistant_run_continue_sse --lib` passed, 1 test;
  - `cargo test -p platform-api assistant_run_live --lib` passed, 1 test;
  - `cargo test -p platform-api assistant_run_continue --lib` passed, 4 tests;
  - `cargo test -p platform-api external_channel_stream_resume --lib` passed, 1 test;
  - `cargo test -p platform-api external_channel_public_stream --lib` passed, 2 tests;
  - `cargo test -p platform-api generic_chat_page_event_stream_can_emit_live_answer_delta_without_final_duplication --lib` passed, 1 test;
  - `cargo check -p platform-api` passed.
- Notes:
  - `assistant_run_streaming` and `external_channel_streaming` are currently plan labels, not active test filters; the local regression filters above are the effective coverage.
- Remaining:
  - run a browser/local main-site streaming smoke with `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true`;
  - run 8-server private `scripts/smoke/external-channel-streaming-10way.mjs` with active bearer;
  - record whether 8-server should keep third-party answer deltas enabled or only stream progress/artifacts until the private smoke passes.

### 2026-06-06 8-Server Deployment Receipt For Main Gap Slice

- Local pushed commit:
  - `1db91f6 Add main-site continue live streaming`.
- 8-server deployment command:
  - `ssh 8服务器 "set -e; cd /srv/aiv3/repo; git pull --ff-only; CC=clang CXX=clang++ cargo build --release -p platform-api -p retrieval-worker; sudo systemctl restart aiv3-platform-api.service aiv3-retrieval-worker.service; sleep 5; systemctl is-active aiv3-platform-api.service; systemctl is-active aiv3-retrieval-worker.service; git rev-parse --short HEAD"`.
- Result:
  - server fast-forwarded from `03458d310` to `1db91f69c`;
  - `platform-api` and `retrieval-worker` release builds succeeded;
  - `aiv3-platform-api.service` active;
  - `aiv3-retrieval-worker.service` active;
  - confirmed remote head: `1db91f69c3fd`.
- 8-server test commands:
  - `CC=clang CXX=clang++ cargo test -p storage document_canonical_enrichment --lib`;
  - `CC=clang CXX=clang++ cargo test -p retrieval-worker --bin document-enrichment-worker`;
  - both passed on 8 server.
- Migration evidence:
  - platform startup and one-shot worker logs confirmed the new document fingerprint columns, `document_content_fingerprints`, `document_enrichment_runs`, and their indexes already exist.
- Fingerprint backfill dry-run:
  - command shape: `./target/release/document-fingerprint-backfill --limit 20 --dry-run --pretty` with 8-server platform env loaded;
  - `candidate_count=20`;
  - `would_record_count=14`;
  - `skipped_count=6`;
  - `recorded_count=0`;
  - `duplicate_count=0`.
- Enrichment worker one-shot:
  - command shape: `DOCUMENT_ENRICHMENT_WORKER_ONCE=true DOCUMENT_ENRICHMENT_WORKER_MAX_RUNS=1 ./target/release/document-enrichment-worker` with 8-server platform env loaded;
  - worker startup succeeded and confirmed schema availability;
  - current `document_enrichment_runs` status query returned no rows, so no enrichment task was claimed;
  - current fingerprint coverage query returned `0/1835` documents with `content_sha256`, because only a dry-run backfill has been executed.
- Runtime flag audit:
  - `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED` is present;
  - `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED` is present;
  - no `DOCUMENT_ENRICHMENT*` runtime flag was visible in the audited env files;
  - no `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED` runtime flag was visible in the audited env files.
- Interpretation:
  - the deploy, migration, service restart, binary availability, dry-run backfill, and one-shot worker startup are verified on 8 server;
  - production document enrichment is still not enabled and existing documents are not yet backfilled;
  - the next safe step is either a limited non-dry-run fingerprint backfill plus one reviewed enrichment enqueue, or an explicit defer until private document-quality smoke credentials are available.

### 2026-06-06 8-Server Streaming And Data-Ingestion Routing Receipt

- Follow-up pushed/deployed commits:
  - `c121072 Prioritize data ingestion fixed task routing`;
  - `eea4f99 Cover data ingestion staging trigger terms`.
- 8-server deployment command shape:
  - `ssh 8服务器 "set -e; cd /srv/aiv3/repo; git pull --ff-only; CC=clang CXX=clang++ cargo build --release -p platform-api; sudo systemctl restart aiv3-platform-api.service; sleep 5; systemctl is-active aiv3-platform-api.service; git rev-parse --short=12 HEAD"`.
- Result:
  - `platform-api` release build succeeded;
  - `aiv3-platform-api.service` active;
  - confirmed remote head: `eea4f995968a`.

Third-party streaming smoke:

- Command shape:
  - `node scripts/smoke/external-channel-streaming-10way.mjs --base-url https://v3.elepcloud.com --connection-id generic-chat-main --timeout-ms 180000`.
- Bearer source:
  - active `generic-chat-main` external-channel connection, loaded server-side/local shell and not printed.
- 8-server receipt:
  - `/srv/aiv3/repo/target/external-channel-streaming-10way-smoke/20260606020619.json`.
- Result:
  - ordinary tasks: 10;
  - static-page tasks: 3;
  - reconnect tasks: 2;
  - total tasks: 15;
  - OK: 15;
  - failed: 0;
  - duplicate final messages: 0;
  - artifact links: 3;
  - P50 latency: 3235 ms;
  - P95 latency: 6015 ms;
  - max latency: 6015 ms.

Data-ingestion live readiness smoke:

- Command shape on 8 server:
  - `DATA_INGESTION_LIVE_SMOKE_DATABASE_URL="$PLATFORM_DATABASE_URL" DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY="hy-sql-traffic-area" DATA_INGESTION_LIVE_SMOKE_API_BASE="http://127.0.0.1:3000" bash scripts/run-data-ingestion-staging-live-smoke.sh`.
- 8-server receipts:
  - `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T020416Z.json`;
  - `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T020416Z.md`.
- Result:
  - source exists and is enabled;
  - connection env reference is present;
  - mapped tables: 6;
  - succeeded sync exists;
  - latest sync is failed and remains an attention signal;
  - dataset count: 3;
  - ready dataset count: 2;
  - default dataset ready: no;
  - question/report ready: yes;
  - safety flags: source database read false, writes allowed false, raw credentials printed false, raw table dump allowed false.

Data-ingestion external fixed-task smoke:

- Local report:
  - `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T023412Z.json`;
  - `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T023412Z.md`.
- AssistantRun:
  - `b43c6169-7de8-4430-96ce-8e754824520f`.
- Codex Host task:
  - `6bf8b409-6d53-4ec9-b1cf-6a29e2d80ec6`.
- Execution:
  - `544f8303-9a92-4c0d-8a34-10e9ba4ff84f`.
- Result:
  - accepted: true;
  - selected source count: 1;
  - initial status included `data_ingestion_analysis_queued`;
  - later status reached `data_ingestion_analysis_running`;
  - server events then reached `assistant_run.data_ingestion_analysis_completed`;
  - third-party reply returned `task_status=data_ingestion_analysis_completed`;
  - card type `v3_data_ingestion_analysis_result`;
  - staging plan type `v3_data_ingestion_staging_plan`;
  - plan id `staging-plan-544f8303-9a92-4c0d-8a34-10e9ba4ff84f`;
  - `human_review_required=true`;
  - `staging_spec_available=true`;
  - output artifact count: 1;
  - safe plan summary: source count 1, table count 1, field mapping count 1, validation count 1;
  - raw credentials present: false.
- Confirmation gap:
  - unauthenticated internal confirm attempt returned HTTP 404 with `assistant_run_not_found`;
  - root cause is operator visibility/session scope, not a third-party public-field contract issue;
  - next implementation slice should add or obtain a controlled operator-auth confirmation path for external-channel staging plans, then run one guarded sync receipt.

### 2026-06-06 Local Operator Confirmation Route Upgrade

- Files changed:
  - `crates/platform-api/src/lib.rs`.
- Behavior:
  - data-ingestion staging plan confirm/sync routes now load the run through a narrow owner-or-operator path;
  - owner-visible runs keep the previous owner-based behavior;
  - external-channel runs owned by the third-party system account can be confirmed/synced by a configured model-gateway operator;
  - anonymous callers and ordinary signed-in non-owner users still receive `assistant_run_not_found`;
  - sync can read the plan-created private staging dataset for that operator-controlled external-channel flow;
  - third-party public URLs, auth, request fields, and response fields were not changed.
- Local verification:
  - `cargo fmt --check -p platform-api` passed;
  - `cargo test -p platform-api data_ingestion --lib` passed, 15 tests;
  - `cargo test -p platform-api external_source_sync --lib` passed, 6 tests;
  - `cargo check -p platform-api` passed;
  - `bash scripts/run-data-ingestion-staging-sync-smoke.sh` passed.
- Smoke receipt:
  - `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T025141Z.json`;
  - `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T025141Z.md`;
  - live readiness self-test `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T025402Z.json`.
- Remaining:
  - deploy this code to 8 server;
  - use an authenticated operator session to confirm one reviewed external-channel staging plan and start one guarded source sync;
  - record synced dataset documents, chunks, retrieval evidence, and safe reply statuses without printing credentials or raw table rows.

### 2026-06-06 8-Server Operator Confirm/Sync Duplicate-Key Receipt

- Deployed commit before this receipt:
  - `688643f780f3`.
- Services checked:
  - `aiv3-platform-api.service` active;
  - `aiv3-codex-host-agent.service` active.
- External data-ingestion analysis smoke:
  - AssistantRun `de6755e0-b490-4665-901b-8847b7a0081b`;
  - local report `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T032231Z.json`;
  - terminal result succeeded with a confirmable staging plan.
- Operator confirmation:
  - plan id `staging-plan-ca9e6b0e-7f16-4fc0-b979-e1ad63afba06`;
  - source id `hy-sql-traffic-area`;
  - confirm returned HTTP 200;
  - accepted true;
  - dataset id `ac7bb786-3ffb-40e2-bade-9f70d5fb4764`;
  - created dataset true;
  - production write allowed false.
- Guarded sync:
  - first sync without `source_id` returned HTTP 400 with `data_ingestion_staging_database_source_ambiguous`, as expected for multiple in-plan sources;
  - retry with `source_id=hy-sql-traffic-area` returned HTTP 202;
  - sync run id `c37d9419-bacf-420b-8add-1fcfac4f02f2`;
  - initial sync status running;
  - deduplicated false;
  - production write allowed false.
- Terminal failure observed from sanitized AssistantRun events:
  - workflow stage `failed`;
  - workflow status `failed`;
  - failure kind `index_external_retrieval`;
  - sanitized error: duplicate key value violates unique constraint `retrieval_evidences_execution_id_document_chunk_id_key`.
- Interpretation:
  - operator confirmation and guarded sync routing are live on 8 server;
  - the remaining P0 blocker is retrieval evidence indexing idempotency, not the third-party public API contract;
  - no bearer token, session token, database URL, source credential, or raw customer row was recorded.

### 2026-06-06 Local Retrieval Evidence Idempotency Slice

- Files changed:
  - `crates/storage/src/lib.rs`;
  - `crates/platform-api/src/lib.rs`;
  - `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`.
- Behavior:
  - `PgRetrievalEvidenceRepository::create_many` now upserts on `(execution_id, document_chunk_id)`;
  - retrying the same indexing execution and chunk reuses the existing `retrieval_evidences.id`;
  - retry metadata such as dataset id, document id, chunk index, source locator, excerpt, summary, payload filter key, embedding model, recall score, evidence manifest, and created time is refreshed from the retry input;
  - the upsert is constrained by `tenant_id` and does not change third-party public URLs, auth, request fields, or response fields.
- Local verification:
  - `cargo fmt --check -p storage -p retrieval-worker -p platform-api` passed;
  - `cargo test -p storage retrieval_evidence --lib` passed, 2 tests;
  - `cargo test -p retrieval-worker external --lib` had no matching tests, so `cargo test -p retrieval-worker --lib` was also run and passed, 6 tests;
  - `cargo test -p platform-api retrieval_evidence_create_many_is_idempotent_for_same_execution_chunk --lib -- --nocapture` compiled and was skipped by the local DB guard because `PLATFORM_DATABASE_URL` points at shared `ai_data_platform_v3`; the guard was not bypassed;
  - `cargo test -p platform-api data_ingestion --lib` passed, 15 tests;
  - `cargo test -p platform-api external_source_sync --lib` passed, 6 tests;
  - `cargo check -p platform-api -p retrieval-worker` passed;
  - `bash scripts/run-data-ingestion-staging-sync-smoke.sh` passed.
- Smoke receipt:
  - JSON `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T034221Z.json`;
  - Markdown `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T034221Z.md`;
  - live readiness self-test `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T034429Z.json`.
- Remaining:
  - commit and push the idempotency slice;
  - deploy to 8 server;
  - rerun the guarded operator sync with explicit `source_id=hy-sql-traffic-area` and `force=true`;
  - record whether the sync completes or fails for a new non-duplicate root cause.

### 2026-06-06 Main-Site And Zip Local Fingerprint Capture

- Files changed:
  - `crates/platform-api/src/lib.rs`.
- Behavior:
  - `/v1/documents` records SHA-256 and byte size when the submitted `object_key` resolves to a local file;
  - unreadable, remote, or oversized object keys are skipped without blocking document registration;
  - zip archive child documents record SHA-256 and byte size after extraction and before child ingest workflows are queued;
  - the same storage helper keeps new document IDs stable while assigning canonical/duplicate fingerprint state.
- Local verification:
  - `cargo fmt --check -p platform-api -p storage` passed.
  - `cargo test -p platform-api register_document_records_local_content_fingerprint --lib` passed.
  - `cargo test -p platform-api create_zip_document_ingest_records_child_content_fingerprint --lib` passed.
  - `cargo test -p platform-api external_document_parse --lib` passed.
  - `cargo test -p storage document_canonical_enrichment --lib` passed.
  - `cargo check -p platform-api` passed.
- Remaining:
  - 8-server migration rollout and live upload/parse smoke;
  - existing-document fingerprint backfill dry-run/execution on 8 server;
  - canonical read-through in retrieval/facts.
