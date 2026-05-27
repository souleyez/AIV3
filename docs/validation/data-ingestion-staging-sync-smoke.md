# Data Ingestion Staging Sync Smoke

This smoke validates the reviewed data-ingestion path after `data_ingestion_analysis` has produced a `v3_data_ingestion_staging_plan`.

The contract is:

- Third-party request fields do not change.
- A reviewed staging plan can create or reuse a private V3 staging dataset.
- The confirmed plan can start `ExternalSourceSync` only for database sources listed in the plan.
- Repeat sync clicks are deduplicated by default.
- V3 records model-visible AssistantRun events for sync started, running, completed, and failed states.
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
- The script writes JSON and Markdown reports under `target/data-ingestion-staging-sync-smoke/`.
- If Node/npm is not available on a deployment target, set `DATA_INGESTION_STAGING_SYNC_SMOKE_SKIP_GUIDE_CHECK=true` to skip only the generated public guide check.

## 8-Server Manual Smoke

After deployment:

1. Submit a data-ingestion analysis request that includes a selected database source or document/data source scope.
2. Poll the run reply until `reply.card.staging_plan.type=v3_data_ingestion_staging_plan`.
3. Call the internal confirm route with the returned `assistant_run_id` and `plan_id`.
4. Confirm the reply moves to `data_ingestion_staging_dataset_ready` with `dataset_id`, `dataset_key`, and `imported_row_count=0`.
5. Call the internal sync route. If the plan has multiple database sources, pass `source_id`.
6. Confirm the reply moves through `data_ingestion_staging_sync_started` and then worker-driven running/completed or failed states.
7. Confirm the target staging dataset contains database-derived documents, chunks, and retrieval evidence before asking/reporting from it.

## 2026-05-27 8-Server Pre-Deploy Probe

Read-only probe against `8服务器` showed `/srv/aiv3/repo` at `50fd00d18` with the core V3 services active. The local staging confirm/sync route and workflow status changes in this branch had not yet been deployed, so the real database-source staging sync smoke remains pending until release.

## Safety Notes

- Use only V3 stored database-source configuration and server-side env references.
- Do not paste or store raw database URLs or passwords in smoke notes.
- Do not enable production table writes or schema mutation for this smoke.
- If sync fails, capture only `sync_run_id`, `workflow_stage`, `workflow_status`, `failure_kind`, and sanitized `last_error`.
