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
| P0 Gate A: 20-way concurrency | in progress | Read-only 8-server queue/status baseline recorded. Requires private bearer/cookie 8-server smoke. Local environment currently has no `EXTERNAL_CHANNEL_SMOKE_BEARER`, `EXTERNAL_REPORT_EXPORT_SMOKE_BEARER`, `V3_EXTERNAL_CHANNEL_BEARER_TOKEN`, `DATAMAX_EXTERNAL_CHANNEL_BEARER_TOKEN`, `MAIN_CHAT_SMOKE_DATASET_ID`, `MAIN_CHAT_SMOKE_COOKIE`, `MAIN_CHAT_SMOKE_BEARER`, `STATIC_PAGE_5WAY_BEARER`, or `STATIC_PAGE_5WAY_DATASET_EXTERNAL_IDS`. |
| P0 Gate B: report/static-page operations | in progress | Accepted-template reuse and Xinbai template contract validated locally/publicly on 2026-06-06. Production low-load prewarm is not enabled because `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED` is unset on 8 server. See `external-capability-routing-smoke.md` and `external-report-export-smoke.md` for latest full bearer-backed report smoke. |
| P1 Gate C: background enterprise memory | in progress | Storage schema phase 1 implemented locally for document fingerprints, canonical aliases, and enrichment runs. Third-party parse, main-site local register, and zip child-document creation now persist SHA-256/size and canonical fingerprint rows when bytes/files are available. A dry-run capable existing-document fingerprint backfill tool exists. 8-server migration/backfill rollout and canonical read-through remain pending. |
| P1 Gate D: low-quality answer recovery | pending | Hard gate remains disabled; passive fixed-scope autofix loop needs implementation and validation. |
| P1 Gate E: confirmed data ingestion | pending | `data_ingestion_analysis` terminal smoke exists; confirmed staging-to-dataset sync still needs closure and validation. |

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
  - canonical read-through in retrieval/facts.

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
  - canonical read-through in retrieval/facts.

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
