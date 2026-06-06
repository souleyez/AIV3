# Data Ingestion Staging Sync Smoke

This smoke validates the reviewed data-ingestion path after `data_ingestion_analysis` has produced a `v3_data_ingestion_staging_plan`.

The contract is:

- Third-party request fields do not change.
- A reviewed staging plan can create or reuse a private DataMax staging dataset.
- The confirmed plan can start `ExternalSourceSync` only for database sources listed in the plan.
- Repeat sync clicks are deduplicated by default.
- DataMax records model-visible AssistantRun events for sync started, running, completed, and failed states.
- Raw database URLs, credentials, full table dumps, schema mutation, and production writes remain blocked.

## Local Verification

Run:

```powershell
bash scripts/run-data-ingestion-staging-sync-smoke.sh
```

Expected:

- `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/confirm` creates or reuses a private staging dataset.
- `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/sync` refuses unconfirmed plans.
- The sync route starts `ExternalSourceSync` into the confirmed staging dataset.
- AssistantRun polling can surface:
  - `data_ingestion_staging_dataset_ready`;
  - `data_ingestion_staging_sync_started`;
  - `data_ingestion_staging_sync_running`;
  - `data_ingestion_staging_sync_completed`;
  - `data_ingestion_staging_sync_failed`.
- Worker unit tests confirm existing database/source materialization stages still parse, ingest, chunk, and index external-source documents.
- The local sync smoke also runs the live-source readiness report builder in `DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true` mode, unless `DATA_INGESTION_STAGING_SYNC_SMOKE_SKIP_LIVE_SELF_TEST=true` is set.
- The script writes JSON and Markdown reports under `target/data-ingestion-staging-sync-smoke/`.
- If Node/npm is not available on a deployment target, set `DATA_INGESTION_STAGING_SYNC_SMOKE_SKIP_GUIDE_CHECK=true` to skip only the generated public guide check.

## 2026-06-06 Local Confirmation-And-Sync Contract Smoke

- Command: `bash scripts/run-data-ingestion-staging-sync-smoke.sh`
- Result: passed.
- Receipt JSON: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T012507Z.json`
- Receipt Markdown: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T012507Z.md`
- Live readiness self-test JSON: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T012938Z.json`
- Live readiness self-test Markdown: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T012938Z.md`

Passed checks:

- `platform-api data-ingestion analysis, staging plan, confirm, and sync contract`
- `platform-api ExternalSourceSync contract`
- `external-source-worker source materialization`
- `ingest-worker source document ingestion`
- `retrieval-worker source indexing`
- `live database-source readiness report self-test`
- `pure third-party guide HTML contract`

Safety result:

- request fields changed: false;
- production write allowed: false;
- schema mutation allowed: false;
- raw database credentials allowed: false;
- raw table dump allowed: false;
- sync requires confirmed plan: true;
- sync source must be in plan: true;
- repeat sync clicks deduplicated: true.

## 2026-06-06 Local Operator Confirmation Route Upgrade

- Command: `bash scripts/run-data-ingestion-staging-sync-smoke.sh`
- Result: passed.
- Receipt JSON: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T025141Z.json`
- Receipt Markdown: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T025141Z.md`
- Live readiness self-test JSON: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T025402Z.json`
- Live readiness self-test Markdown: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T025402Z.md`

Additional route coverage:

- external-channel staging plan owned by a third-party system user can be confirmed by a configured model-gateway operator;
- the same operator can start sync for the confirmed private staging dataset;
- anonymous callers still receive `assistant_run_not_found`;
- ordinary signed-in non-owner users still receive `assistant_run_not_found`;
- production write allowed: false;
- raw credential exposure: false;
- third-party public URL/auth/request/response contract changed: false.

## 8-Server Manual Smoke

After deployment:

1. Submit a data-ingestion analysis request that includes a selected database source or document/data source scope.
2. Poll the run reply until `reply.card.staging_plan.type=v3_data_ingestion_staging_plan`.
3. Call the internal confirm route with the returned `assistant_run_id` and `plan_id`.
4. Confirm the reply moves to `data_ingestion_staging_dataset_ready` with `dataset_id`, `dataset_key`, and `imported_row_count=0`.
5. Call the internal sync route. If the plan has multiple database sources, pass `source_id`.
6. Confirm the reply moves through `data_ingestion_staging_sync_started` and then worker-driven running/completed or failed states.
7. Confirm the target staging dataset contains database-derived documents, chunks, and retrieval evidence before asking/reporting from it.

## 8-Server Live Source Readiness Smoke

Run on the deployment target after release:

```bash
bash scripts/run-data-ingestion-staging-live-smoke.sh
```

Useful options:

- `DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY=hy-sql-traffic-area` selects the stored database source.
- `DATA_INGESTION_LIVE_SMOKE_REQUIRE_DEFAULT_READY=true` makes the smoke fail unless the source default dataset is already indexed.
- `DATA_INGESTION_LIVE_SMOKE_API_BASE=http://127.0.0.1:3000` selects the local platform API used only for the selected-source status snapshot.
- `DATA_INGESTION_LIVE_SMOKE_REPO_ROOT=/srv/aiv3/repo` can be used when piping the script through SSH before it is deployed.
- `DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true` runs the report builder against synthetic DataMax status fixtures without requiring Postgres or the platform API; use this only to verify the smoke/report logic itself.

The live smoke is non-destructive. It reads only DataMax PostgreSQL and the internal selected-source status route. It does not connect to the source/customer database, does not start a sync, does not print raw credentials, and does not dump source rows.

Expected report signals:

- Source exists, is enabled, and has a server-side connection env reference.
- At least one succeeded sync run exists.
- At least one source-derived dataset has indexed documents and indexed chunks.
- Question/report readiness is reported separately and requires a source-derived dataset with indexed documents, indexed chunks, retrieval evidence, and at least one ready source table.
- The report includes safe suggested follow-up questions and the local static-page report smoke command, but it does not execute those customer-facing questions automatically.
- The default dataset readiness is reported separately from alternate ready datasets, so an empty newly-bound default dataset is visible without hiding older ready data.
- Latest failed sync is a warning/attention signal, not proof that historical indexed data disappeared.

## 2026-05-27 8-Server Post-Deploy Probe

Read-only probe against `8服务器` after release showed `/srv/aiv3/repo` at `5b5df7e` with the core DataMax services active. The stored MySQL source `hy-sql-traffic-area` exists, has two earlier succeeded content sync runs, and has an older source-derived smoke dataset with 50 indexed database documents, 50 indexed chunks, and 50 retrieval evidence rows. The current default dataset `新百经营分析` is present but has no indexed database-source documents yet, and the latest full sync run is marked failed because a slow metadata query was operator-cancelled. This is exactly why the live smoke separates default dataset readiness, alternate ready datasets, and latest sync failure.

## 2026-06-06 8-Server Live Source Readiness Result

- Environment: `8服务器`.
- Source key: `hy-sql-traffic-area`.
- Command shape:
  - `DATA_INGESTION_LIVE_SMOKE_DATABASE_URL="$PLATFORM_DATABASE_URL" DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY="hy-sql-traffic-area" DATA_INGESTION_LIVE_SMOKE_API_BASE="http://127.0.0.1:3000" bash scripts/run-data-ingestion-staging-live-smoke.sh`
- Report JSON:
  - `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T020416Z.json`
- Report Markdown:
  - `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T020416Z.md`
- Result: passed as a read-only readiness smoke.

Observed safe signals:

- source exists: true;
- source enabled: true;
- connection env reference present: true;
- mapped table count: 6;
- succeeded sync exists: true;
- latest sync failed: true;
- dataset count: 3;
- ready dataset count: 2;
- default dataset ready: no;
- question/report ready: yes.

Ready datasets:

- `hy-sql-traffic-area-smoke-20260521`: documents 50, chunks 50, retrieval evidence 50, default no.
- `external-source-third-party-source-main-dataset-64fff6c8-10e2-4ee8-8243-23166cce3abc`: documents 1, chunks 1, retrieval evidence 1, default no.

API snapshot:

- dataset signal: `no_documents`;
- sync signal: `sync_failed`;
- health: blocking;
- health codes: `mapped_tables_without_documents`, `latest_sync_failed`.

Safety result:

- reads DataMax PostgreSQL only: true;
- source database read: false;
- writes allowed: false;
- raw credentials printed: false;
- raw table dump allowed: false.

Interpretation:

- The stored source has enough historical indexed data for question/report smoke.
- The current default dataset still needs a confirmed sync or rebinding before it can be treated as ready.
- Latest failed sync should stay visible as an operator attention item and should not erase the fact that alternate ready datasets exist.

## 2026-06-06 Identity Audit Extension

- Commit: `25fb0ee`.
- Changed script: `scripts/run-data-ingestion-staging-live-smoke.sh`.
- Purpose:
  - add a reusable read-only `latest_sync_identity_audit` section;
  - report configured identity columns, latest-sync source rows, unique materialized documents, collapsed duplicate rows, and current document counts by table;
  - make source-row vs entity/store-level materialization gaps visible before database-derived reporting.
- Local verification:
  - command: `DATA_INGESTION_STAGING_SYNC_SMOKE_SKIP_GUIDE_CHECK=true bash scripts/run-data-ingestion-staging-sync-smoke.sh`;
  - result: passed;
  - receipt JSON: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T080441Z.json`;
  - live self-test receipt JSON: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T080659Z.json`.
- 8-server read-only verification:
  - `/srv/aiv3/repo` commit: `25fb0ee9d0cb`;
  - source key: `hy-sql-traffic-area`;
  - receipt JSON: `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T080856Z.json`;
  - receipt Markdown: `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T080856Z.md`;
  - result: passed.
- 8-server sanitized audit result:
  - latest sync: `e9da6483-5705-416e-bdc2-a1cc219f6566`;
  - collapsed table count: 2;
  - collapsed duplicate rows: 193;
  - `bi_contract_warning`: source rows 100, unique documents 6, collapsed rows 94;
  - `bi_rentsales_detail`: source rows 100, unique documents 1, collapsed rows 99;
  - other latest-sync tables showed no source-row collapse.
- Safety result:
  - DataMax PostgreSQL read only;
  - customer/source database read: false;
  - writes allowed: false;
  - raw credentials printed: false;
  - raw table dump allowed: false.
- Next staging decision:
  - if source-row-level completeness is required for Xinbai reports, test a staging-only mapping with finer row discriminator columns for the two collapsed fact/detail tables before changing production mappings.

## 2026-06-06 8-Server Operator Confirm/Sync Result Before Idempotency Fix

- Environment: `8服务器`.
- Deployed commit: `688643f780f3`.
- Analysis AssistantRun: `de6755e0-b490-4665-901b-8847b7a0081b`.
- Plan id: `staging-plan-ca9e6b0e-7f16-4fc0-b979-e1ad63afba06`.
- Source id: `hy-sql-traffic-area`.
- Confirm result:
  - HTTP 200;
  - accepted true;
  - dataset id `ac7bb786-3ffb-40e2-bade-9f70d5fb4764`;
  - created dataset true;
  - production write allowed false.
- Sync result:
  - initial request without `source_id` returned HTTP 400 `data_ingestion_staging_database_source_ambiguous`;
  - retry with `source_id=hy-sql-traffic-area` returned HTTP 202;
  - sync run id `c37d9419-bacf-420b-8add-1fcfac4f02f2`;
  - initial status running;
  - deduplicated false;
  - production write allowed false.
- Terminal failure:
  - workflow stage `failed`;
  - workflow status `failed`;
  - failure kind `index_external_retrieval`;
  - sanitized error references duplicate key `retrieval_evidences_execution_id_document_chunk_id_key`.
- Interpretation:
  - human/operator confirmation and guarded sync startup are live;
  - sync source validation works;
  - the next fix must make retrieval evidence indexing idempotent before this flow can be called complete.

## 2026-06-06 Local Retrieval Evidence Idempotency Coverage

- Files changed:
  - `crates/storage/src/lib.rs`;
  - `crates/platform-api/src/lib.rs`.
- Behavior:
  - `retrieval_evidences` writes now upsert on `(execution_id, document_chunk_id)`;
  - retrying the same execution/chunk refreshes the evidence row instead of creating a duplicate or failing the sync;
  - public third-party request and response fields are unchanged.
- Local commands:
  - `cargo fmt --check -p storage -p retrieval-worker -p platform-api`;
  - `cargo test -p storage retrieval_evidence --lib`;
  - `cargo test -p retrieval-worker external --lib`;
  - `cargo test -p retrieval-worker --lib`;
  - `cargo test -p platform-api retrieval_evidence_create_many_is_idempotent_for_same_execution_chunk --lib -- --nocapture`;
  - `cargo test -p platform-api data_ingestion --lib`;
  - `cargo test -p platform-api external_source_sync --lib`;
  - `cargo check -p platform-api -p retrieval-worker`;
  - `bash scripts/run-data-ingestion-staging-sync-smoke.sh`.
- Results:
  - format check passed;
  - storage retrieval evidence tests passed, 2 tests;
  - `retrieval-worker external` matched 0 tests; full `retrieval-worker --lib` passed, 6 tests;
  - platform DB idempotency test compiled but was skipped by the local DB guard because the configured database is shared `ai_data_platform_v3`; the guard was not bypassed;
  - platform data-ingestion tests passed, 15 tests;
  - platform external-source sync tests passed, 6 tests;
  - platform/retrieval compile check passed;
  - staging sync smoke passed.
- Smoke receipts:
  - JSON `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T034221Z.json`;
  - Markdown `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T034221Z.md`;
  - live readiness self-test JSON `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T034429Z.json`.
- Remaining:
  - deploy this slice to 8 server;
  - rerun guarded sync with explicit `source_id=hy-sql-traffic-area` and `force=true`;
  - record document, chunk, retrieval evidence counts or a new sanitized non-duplicate failure.

## 2026-06-06 8-Server Idempotency Retry Result

- Environment: `8服务器`.
- Deployed commit: `8fd0a1d69df0`.
- Build/restart:
  - `platform-api` release build succeeded;
  - `retrieval-worker` release build succeeded;
  - `aiv3-platform-api.service` active;
  - `aiv3-retrieval-worker.service` active.
- Reused reviewed staging flow:
  - AssistantRun `de6755e0-b490-4665-901b-8847b7a0081b`;
  - plan id `staging-plan-ca9e6b0e-7f16-4fc0-b979-e1ad63afba06`;
  - source id `hy-sql-traffic-area`;
  - dataset id `ac7bb786-3ffb-40e2-bade-9f70d5fb4764`.
- Confirm result:
  - HTTP 200;
  - accepted true;
  - created dataset false;
  - production write allowed false.
- Sync retry body:
  - `source_id=hy-sql-traffic-area`;
  - `sync_kind=full`;
  - `force=true`;
  - checkpoint `operator_smoke=2026-06-06-idempotent-retry`.
- Sync result:
  - HTTP 202;
  - sync run id `0f75e5ef-130a-4ba8-a4c6-efe880db5ce2`;
  - deduplicated false;
  - production write allowed false;
  - AssistantRun events reached `workflow_stage=completed` and `workflow_status=succeeded`;
  - duplicate key `retrieval_evidences_execution_id_document_chunk_id_key` did not recur.
- Current confirmed staging dataset state:
  - documents 384;
  - chunks 384;
  - retrieval evidence rows 384;
  - document lifecycle distribution: `indexed=384`.
- Sync run stored counts:
  - status `succeeded`;
  - failure kind empty;
  - row count 577;
  - documents ingested 577;
  - chunks ingested 577;
  - chunks indexed 577;
  - retrieval evidences indexed 577;
  - failed row count 0.
- Count-audit note:
  - the successful sync's processed/indexed counters report 577 while the current staging dataset has 384 unique documents/chunks/evidence rows;
  - current unique table distribution is `bi_contract_warning=6`, `bi_oa_zulinhetong=100`, `bi_oa_zulinhetonggudingzujin=100`, `bi_oa_zulinhetongtichengzujin=100`, `bi_rentsales_detail=1`, `nwstore=77`;
  - this should be audited as a counting/materialization semantics issue before claiming exact row-to-document parity.
- Safety result:
  - public third-party contract changed: false;
  - production write allowed: false;
  - schema mutation allowed: false;
  - raw credentials printed: false;
  - raw source rows printed: false.

## 2026-06-06 Current-Head Read-Only Row Identity Audit

- Environment: `8服务器`.
- Repository state:
  - `/srv/aiv3/repo` at `7be4111eb85a`;
  - `git status --short --branch` showed `## main...origin/main` and the pre-existing untracked `mode` file.
- Command shape:
  - source `/etc/aiv3/aiv3.env`;
  - pass `PLATFORM_DATABASE_URL` to `DATA_INGESTION_LIVE_SMOKE_DATABASE_URL` without printing the value;
  - run `DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY=hy-sql-traffic-area DATA_INGESTION_LIVE_SMOKE_API_BASE=http://127.0.0.1:3000 bash scripts/run-data-ingestion-staging-live-smoke.sh`.
- Receipts:
  - JSON `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T100024Z.json`;
  - Markdown `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T100024Z.md`.
- Result:
  - smoke status passed;
  - source exists and is enabled;
  - mapped table count 6;
  - succeeded sync exists;
  - latest sync is not failed;
  - dataset count 5;
  - ready dataset count 4;
  - default dataset ready false;
  - question/report ready true.
- Latest sync identity audit:
  - latest sync `e9da6483-5705-416e-bdc2-a1cc219f6566`;
  - collapsed table count 2;
  - collapsed duplicate rows 193;
  - `bi_contract_warning`: identity columns `parentcode`, `storecode`, `txdate`; source rows 100; unique docs 6; collapsed rows 94; current docs 24;
  - `bi_rentsales_detail`: identity columns `storecode`, `contract_no`, `contract_startdate`; source rows 100; unique docs 1; collapsed rows 99; current docs 6;
  - `bi_oa_zulinhetong`, `bi_oa_zulinhetonggudingzujin`, `bi_oa_zulinhetongtichengzujin`, and `nwstore` did not show latest-sync row collapse.
- Decision point:
  - if these two tables are meant to represent store/entity snapshots, current mapping can be documented as entity-level;
  - if row-level report completeness is required, do a staging-only discriminator mapping test first and compare source rows, unique documents, chunks, evidence, and Xinbai report behavior before production adoption.
- Safety:
  - reads DataMax PostgreSQL only;
  - does not query the customer/source database;
  - no production writes, schema mutation, raw credentials, raw source rows, full table dump, bearer token, or public third-party contract change.

## Safety Notes

- Use only DataMax stored database-source configuration and server-side env references.
- Do not paste or store raw database URLs or passwords in smoke notes.
- Do not enable production table writes or schema mutation for this smoke.
- If sync fails, capture only `sync_run_id`, `workflow_stage`, `workflow_status`, `failure_kind`, and sanitized `last_error`.

## 2026-06-06 Current-Head Row Identity Audit Refresh

- Environment: `8服务器`.
- Repository state:
  - `/srv/aiv3/repo` at `112cc82e8457`;
  - `git status --short --branch` showed `## main...origin/main` and the pre-existing untracked `mode` file.
- Command shape:
  - source `/etc/aiv3/aiv3.env`;
  - pass `PLATFORM_DATABASE_URL` to `DATA_INGESTION_LIVE_SMOKE_DATABASE_URL` without printing the value;
  - run `DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY=hy-sql-traffic-area DATA_INGESTION_LIVE_SMOKE_API_BASE=http://127.0.0.1:3000 bash scripts/run-data-ingestion-staging-live-smoke.sh`.
- Receipts:
  - JSON `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T123346Z.json`;
  - Markdown `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T123346Z.md`.
- Result:
  - smoke status passed;
  - source exists and is enabled;
  - mapped table count 6;
  - latest sync failed false;
  - dataset count 5;
  - ready dataset count 4;
  - default dataset ready false;
  - question/report ready true.
- Latest sync identity audit:
  - latest sync `e9da6483-5705-416e-bdc2-a1cc219f6566`;
  - collapsed table count 2;
  - collapsed duplicate rows 193;
  - `bi_contract_warning`: identity columns `parentcode`, `storecode`, `txdate`; source rows 100; unique docs 6; collapsed rows 94; current docs 24;
  - `bi_rentsales_detail`: identity columns `storecode`, `contract_no`, `contract_startdate`; source rows 100; unique docs 1; collapsed rows 99; current docs 6.
- Decision point:
  - keep production mapping unchanged for entity/latest-snapshot reporting;
  - if row-level report completeness is required, run a staging-only discriminator mapping test before production adoption.
- Safety:
  - reads DataMax PostgreSQL only;
  - does not query the customer/source database;
  - no production writes, schema mutation, raw credentials, raw source rows, full table dump, bearer token, or public third-party contract change.
