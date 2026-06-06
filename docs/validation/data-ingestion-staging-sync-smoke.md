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

## Safety Notes

- Use only DataMax stored database-source configuration and server-side env references.
- Do not paste or store raw database URLs or passwords in smoke notes.
- Do not enable production table writes or schema mutation for this smoke.
- If sync fails, capture only `sync_run_id`, `workflow_stage`, `workflow_status`, `failure_kind`, and sanitized `last_error`.
