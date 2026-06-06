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
| P0 Gate A: 20-way concurrency | passed for current deploy | Read-only 8-server queue/status baseline recorded. 8-server third-party streaming smoke passed with active bearer: 10 ordinary, 3 static-page, 2 reconnect, 15/15 OK, duplicate final messages 0, P95 6015 ms. External-channel 20-way smoke passed after the latest deploy with 20/20 OK and P95 4036 ms. Main-site 20-way smoke initially exposed a chat-session workflow-start bug; after commit `83e49529b9fd`, rerun passed with 20/20 accepted, 20/20 assistant messages, and P95 15906 ms. Static-page 5-way passed with 5/5 artifacts. Cloudflare fallback guard passed with configured concurrency 2. Document-quality local regression passed. |
| P0 Gate B: report/static-page operations | passed for explicit report/static-page requests | Accepted-template reuse and Xinbai template contract validated locally/publicly on 2026-06-06. 8-server static-page 5-way smoke returned 5/5 artifact links. 8-server report export smoke passed for JSON and SSE, confirmed title `新世界百货经营管理月报表`, focus `取高机会`, one report surface, and accessible `table-data.csv`, `report.ppt`, `report.md`. Production low-load prewarm is not enabled because `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED` is unset on 8 server. Local prewarm safety coverage now proves silent prewarm sources are allowed through the later auto-publish path and do not dispatch customer outbound replies, but a real prewarm consumer/low-load execution smoke remains required before production enablement. |
| P0 Gate C: controlled streaming | passed for current contract | Local stream regressions passed. 8 server has `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true` with provider runtime `rightcode/gpt-5.5`. New reusable smoke `npm run smoke:main-assistant-streaming` passed against `https://v3.elepcloud.com`: new AssistantRun emitted 74 deltas, continue emitted 71 deltas, both ended with exactly one completed event and one done event, and no duplicate final-text delta was detected. |
| P1 Gate C: background enterprise memory | passed for post-ingest rollout | Storage schema phase 1 implemented for document fingerprints, canonical aliases, and enrichment runs. Third-party parse, main-site local register, and zip child-document creation now persist SHA-256/size and canonical fingerprint rows when bytes/files are available. Canonical read-through for chunks/evidence/facts, feature-flagged post-ingest enrichment enqueue, document-level enrichment diagnostics, a standalone low-priority enrichment worker loop, deterministic enrichment kinds for tables/procedures/entities/resumes/spreadsheets, and local aggregate-first answer supply are implemented. On 8 server, the controlled batch recorded one canonical fingerprint and one `fact_index_v2` success. Commit `3171fbb5e47d` is deployed with `DOCUMENT_ENRICHMENT_KINDS` narrowed to five deterministic kinds, `aiv3-document-enrichment-worker.service` active at low priority, and no pending enrichment backlog. A tiny live upload/new-parse smoke then proved all five configured deterministic kinds enqueue and succeed for a newly indexed document. Full existing-document backfill remains disabled. |
| P1 Gate D: low-quality answer recovery | passed for safe-disabled deploy | Passive local implementation and smoke passed on 2026-06-06. Hard gate remains disabled. Production enqueue now requires `CODEX_HOST_TASK_ENABLED=true`, `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED=true`, `CODEX_HOST_TASK_ALLOWLIST` containing `answer_quality_autofix`, and host capability allowlist before live Codex task creation. Dedicated opt-in guard is deployed to 8 server on `3394b0a5f60a`; runtime audit shows the dedicated autofix flag unset and both allowlists excluding `answer_quality_autofix`, so live enqueue remains intentionally disabled until an explicit operator decision. |
| P1 Gate E: confirmed data ingestion | passed for current contract | Local confirmed staging-to-dataset sync smoke passed on 2026-06-06. 8-server live source readiness passed for `hy-sql-traffic-area`. 8-server external data-ingestion analysis completed and returned a `v3_data_ingestion_staging_plan` with `human_review_required=true` and no raw credentials. Operator-confirmation routing is implemented and tested. 8-server authenticated operator confirm/sync smoke succeeded after retrieval-evidence idempotency commit `8fd0a1d`; sync `0f75e5ef-130a-4ba8-a4c6-efe880db5ce2` completed. The 577 vs 384 audit found source-row counts were being reported as materialized/indexed counts when MySQL identity mappings collapsed multiple rows into one document. Commit `8c72144aa5f9` separates source rows, unique materialized documents/chunks/evidence, and collapsed duplicate rows; 8-server re-sync `e9da6483-5705-416e-bdc2-a1cc219f6566` succeeded with source rows 577, unique documents/chunks/evidence 384, and collapsed duplicate rows 193. |
| P1 Gate F: operator observability | passed for deployed page and queue summary | The external integrations page now includes a compact sanitized operations summary for ordinary chat, model lane, workflow backlog, report/static-page jobs, template/artifact state, data ingestion, document enrichment, and low-quality recovery. It reuses existing protected/light endpoints and keeps task/runtime/conversation details lazy-loaded. Local external-integrations helper tests passed, web build passed, and local HTTP smoke returned `200`. 8-server deploy to `2193e0cd5248` succeeded; local/public `/external-integrations` returned `200` with DataMax/运营总览 SSR text; the web queue-stats proxy returned `200` with the configured observability cookie. Model-gateway status remains protected and returned `401 auth_session_required` without a main-system operator session, so an authenticated model-gateway operator smoke remains pending. |

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

### 2026-06-06 External-Channel 20-Way Smoke

- Command:
  - `npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20 --timeout-ms 120000`
- Receipt:
  - `target/external-channel-20way-smoke/20260606035800.json`
- Result:
  - `okCount=20`, `failedCount=0`.
  - Latency: `p50=4739 ms`, `p95=5721 ms`, `max=6453 ms`.
  - SSE included `external_channel.delta` and `external_channel.completed` events.
- Interpretation:
  - The third-party external-channel endpoint can accept and complete 20 concurrent ordinary requests with the private bearer loaded outside the repository.
  - The script's `answeredCount=0` reflects current script/report semantics and was not treated as a failure because all tasks emitted completion events.

### 2026-06-06 Main-Site 20-Way Smoke Failure And Local Fix

- Command:
  - `npm run smoke:main-chat-20way -- --base-url https://v3.elepcloud.com --concurrency 20 --timeout-ms 90000 --poll-timeout-ms 120000`
- Receipt:
  - `target/main-chat-20way-smoke/20260606035856.json`
- Result before fix:
  - `acceptedCount=20`.
  - `assistantMessageCount=0`.
  - `okCount=0`, `failedCount=20`.
  - Poll latency around 124-126 seconds because sessions never received assistant messages.
- 8-server diagnosis:
  - `aiv3-assistant-run-worker.service`, `aiv3-chat-session-worker.service`, `aiv3-platform-api.service`, and `aiv3-retrieval-worker.service` were active.
  - Recent `chat_session_workflow` executions stayed `status=pending`, `stage=queued`.
  - No `chat_session/orchestrate_chat_session` workflow tasks were enqueued.
- Root cause:
  - `create_chat_session` and `append_chat_session_turn` created workflow executions and events but did not apply `WorkflowSignal::Start`.
- Local fix:
  - After session/message/context persistence, both routes now apply `WorkflowSignal::Start` and return the started execution.
  - This keeps the existing public response shape unchanged while moving `workflow_execution.status` to `running`, `stage` to `orchestrate_chat_session`, and enqueueing the worker task.
- Local verification:
  - `cargo fmt --check -p platform-api` passed.
  - `cargo test -p platform-api append_chat_session_turn_creates_user_message_and_new_execution --lib -- --nocapture` completed; the local shared-database guard skipped the DB route assertions as designed.
  - `cargo check -p platform-api` passed.
- Remaining:
  - Completed by the 8-server rollout receipt below.

### 2026-06-06 Chat-Session Workflow-Start Rollout And P0 Smoke

- Commit:
  - `83e49529b9fd` (`Start chat session workflows automatically`).
- Changed behavior:
  - `create_chat_session` and `append_chat_session_turn` now apply `WorkflowSignal::Start` after session/message/context persistence.
  - Public response shape is unchanged.
  - Returned workflow execution now has `status=running`, `stage=orchestrate_chat_session`, and a queued `chat_session/orchestrate_chat_session` task.
- Local verification before deploy:
  - `cargo fmt --check -p platform-api` passed.
  - `cargo test -p platform-api append_chat_session_turn_creates_user_message_and_new_execution --lib -- --nocapture` completed; the local shared-database guard skipped the DB route assertions as designed.
  - `cargo check -p platform-api` passed.
- 8-server deploy:
  - `git pull --ff-only` moved `/srv/aiv3/repo` to `83e49529b9fd`.
  - `CC=clang CXX=clang++ cargo build --release -p platform-api` passed.
  - `aiv3-platform-api.service` restarted and reported `active`.
  - After smoke, these services reported `active`: `aiv3-platform-api.service`, `aiv3-chat-session-worker.service`, `aiv3-assistant-run-worker.service`, `aiv3-static-page-worker.service`, `aiv3-codex-host-agent.service`, and `aiv3-retrieval-worker.service`.
  - Existing untracked 8-server file `mode` was observed and not touched.
- Main-site 20-way rerun:
  - Command: `npm run smoke:main-chat-20way -- --base-url https://v3.elepcloud.com --concurrency 20 --timeout-ms 90000 --poll-timeout-ms 120000`.
  - Receipt: `/srv/aiv3/repo/target/main-chat-20way-smoke/20260606041553.json`.
  - Result: `okCount=20`, `failedCount=0`, `acceptedCount=20`, `assistantMessageCount=20`.
  - Latency: `p50=12487 ms`, `p95=15906 ms`, `max=15963 ms`.
- External-channel 20-way rerun:
  - Command: `npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20 --timeout-ms 120000`.
  - Receipt: `/srv/aiv3/repo/target/external-channel-20way-smoke/20260606042403.json`.
  - Result: `okCount=20`, `failedCount=0`, `completedCount=20`.
  - Latency: `p50=2776 ms`, `p95=4036 ms`, `max=6923 ms`.
  - The script's `answeredCount=0` reflects current script/report semantics and was not treated as a failure because all tasks emitted completion events.
- Static-page 5-way:
  - Command: `npm run smoke:static-page-5way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 5 --timeout-ms 120000 --poll-timeout-ms 300000`.
  - Receipt: `/srv/aiv3/repo/target/static-page-5way-smoke/20260606042000.json`.
  - Result: `okCount=5`, `failedCount=0`, `acceptedCount=5`, `artifactCount=5`, `requireArtifact=true`.
  - Latency: `p50=7068 ms`, `p95=7794 ms`, `max=7794 ms`.
  - Note: the first run used the wrong tenant's duplicate `generic-chat-main` bearer and returned expected HTTP 401. The passing run used the active `local-dev` channel connection token loaded from 8-server configuration and not printed.
- Cloudflare fallback 2-way guard:
  - Command: `npm run smoke:cloudflare-fallback-2way -- --base-url https://v3.elepcloud.com --max-allowed 2 --min-expected 2`.
  - Receipt: `/srv/aiv3/repo/target/cloudflare-fallback-2way-smoke/20260606042140.json`.
  - Result: `ok=true`, `codexConcurrency=2`, `maxRunning=0`, model-gateway status loaded, workflow queue stats loaded.
  - Note: a preliminary run used `auth_method=smoke` for a temporary operator session and correctly failed with `unknown auth session method`; the passing run used `local_key`.
- External report export:
  - Command: `npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --timeout-ms 180000`.
  - Receipt: `/srv/aiv3/repo/target/external-report-export-smoke/20260606042158.json`.
  - Result: `okCount=2`, `failedCount=0`, JSON and SSE passed.
  - Confirmed: title `新世界百货经营管理月报表`, focus `取高机会`, `table-data.csv`, `report.ppt`, and `report.md`.
- Document-quality regression:
  - Command: `powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local`.
  - Receipt: `target/document-quality-smoke/document-quality-smoke-20260606T042304Z-10920.json`.
  - Markdown summary: `target/document-quality-smoke/document-quality-smoke-20260606T042304Z-10920.md`.
  - Result: passed all cases, including one-character PDF, `邓工是谁`, elderly-care procedure facts, resume company statistics, multi-dimensional resume ranking, table-heavy documents, attendance date/work-hour analysis, scanned PDF fallback, smart-home customer-feedback answer quality, and smart-elevator point-list answer quality.

### 2026-06-06 Main-Site AssistantRun Streaming Smoke

- New smoke script:
  - `scripts/smoke/main-assistant-streaming.mjs`.
  - Package command: `npm run smoke:main-assistant-streaming`.
- What it checks:
  - `POST /v1/assistant-runs/stream` emits exactly one `assistant_run.accepted`, one or more `assistant_run.delta`, exactly one `assistant_run.completed`, and exactly one `done`.
  - `POST /v1/assistant-runs/{run_id}/continue/stream` does the same for a continued run.
  - Completed payload includes the assistant response.
  - The concatenated deltas do not look like the final full answer repeated twice.
  - Optional strict mode requires multiple deltas for both create and continue paths.
- Local regressions run before this smoke:
  - `cargo fmt --check -p platform-api` passed.
  - `cargo test -p platform-api assistant_run_sse --lib` passed, 5 tests.
  - `cargo test -p platform-api assistant_run_continue_sse --lib` passed, 1 test.
  - `cargo test -p platform-api assistant_run_live --lib` passed, 1 test.
  - `cargo test -p platform-api assistant_run_continue --lib` passed, 4 tests.
  - `cargo test -p platform-api external_channel_public_stream --lib` passed, 2 tests.
  - `npm --prefix apps/web run build` passed with the existing Next.js tracing warning.
- 8-server runtime precondition:
  - `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true`.
  - `ASSISTANT_RUN_RUNTIME_MODE=provider`.
  - `ASSISTANT_RUN_RUNTIME_PROVIDER=rightcode`.
  - `ASSISTANT_RUN_RUNTIME_MODEL=gpt-5.5`.
  - `LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=active`.
- Public 8-server strict smoke:
  - Command: `npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com --timeout-ms 120000 --require-live-delta --require-multiple-deltas`.
  - Receipt: `target/main-assistant-streaming-smoke/20260606043328.json`.
  - Result: `ok=true`, `createOk=true`, `continueOk=true`.
  - Create path: `createDeltaCount=74`, `createFirstDeltaAtMs=4792`, `createLatencyMs=6508`.
  - Continue path: `continueDeltaCount=71`, `continueFirstDeltaAtMs=2951`, `continueLatencyMs=4733`.
  - AssistantRun id: `d12e4106-e111-4ad4-889c-9a67c6820179`.
- 8-server local rerun after pulling the smoke script:
  - Commit on server: `95a23989b085`.
  - Command: `npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com --timeout-ms 120000 --require-live-delta --require-multiple-deltas`.
  - Receipt: `/srv/aiv3/repo/target/main-assistant-streaming-smoke/20260606043550.json`.
  - Result: `ok=true`, `createOk=true`, `continueOk=true`.
  - Create path: `createDeltaCount=83`, `createFirstDeltaAtMs=1918`, `createLatencyMs=3506`.
  - Continue path: `continueDeltaCount=70`, `continueFirstDeltaAtMs=1697`, `continueLatencyMs=4167`.
  - AssistantRun id: `5996bc97-2860-443b-86c0-d2c1333ebb8d`.
- Useful pre-run note:
  - A previous strict run failed because the default prompt was too short and produced a single delta on one path. The script was adjusted to use longer default create/continue prompts so strict multi-delta mode is stable enough for release gating.

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

### 2026-06-06 Static-Page Prewarm Auto-Publish Safety Slice

- Files changed:
  - `crates/platform-api/src/lib.rs`.
- Behavior:
  - extracted the silent prewarm source constant `external_channel_static_page_template_prewarm_candidate`;
  - allows that prewarm source through the later Image2-preview-to-static-page auto-publish source gate;
  - keeps explicit static-page requests and local chat static-page pipelines on the same auto-publish allowlist;
  - skips third-party outbound reply dispatch for static-page publish success/failure when the draft source refs carry `prewarm.customer_visible=false`.
- Local verification:
  - `cargo fmt --check -p platform-api` passed.
  - `cargo test -p platform-api static_page_template_prewarm --lib` passed, 3 tests.
  - `cargo check -p platform-api` passed.
- Remaining:
  - production `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED` remains unset;
  - a real low-load prewarm consumer/execution smoke is still required before enabling the production flag, otherwise prewarm can become queued work without a proven end-to-end template artifact.

### 2026-06-06 8-Server Static-Page Prewarm Safety Rollout

- Commit deployed:
  - `0dc117d0fafa`.
- Deployment:
  - 8 server fast-forwarded from `e6b6c5ba5` to `0dc117d0f`;
  - `CC=clang CXX=clang++ cargo build --release -p platform-api` passed;
  - `aiv3-platform-api.service` was restarted and returned `active`;
  - repo remains `main...origin/main`; existing untracked `mode` remains untouched.
- 8-server verification:
  - `CC=clang CXX=clang++ cargo test -p platform-api static_page_template_prewarm --lib` passed, 3 tests;
  - `GET http://127.0.0.1:3000/v1/workflow-tasks/queue-stats` returned HTTP 200;
  - `STATIC_PAGE_TEMPLATE_PREWARM_*` runtime env remains unset for `aiv3-platform-api.service`.
- Services checked after deploy:
  - `aiv3-platform-api.service`: active;
  - `aiv3-web.service`: active;
  - `aiv3-static-page-worker.service`: active;
  - `aiv3-codex-host-agent.service`: active;
  - `aiv3-chat-session-worker.service`: active;
  - `aiv3-assistant-run-worker.service`: active;
  - `aiv3-retrieval-worker.service`: active;
  - `aiv3-document-enrichment-worker.service`: active.
- Safety boundary:
  - the deploy does not enable production low-load prewarm;
  - no third-party public URL, auth, request field, or response field changed;
  - the change only prevents future silent prewarm artifacts from being blocked by the auto-publish source gate or accidentally dispatched as customer-visible replies.

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
  - after deployment, run 8-server runtime flag audit and decide whether to explicitly enable live enqueue.

### 2026-06-06 Answer Quality Autofix Dedicated Opt-In Guard

- Files changed:
  - `crates/platform-api/src/lib.rs`;
  - `docs/operations/answer-quality-autofix.md`;
  - `docs/operations/cloudflare-codex-fixed-task-templates.md`;
  - `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`;
  - `docs/validation/datamax-main-gap-closure.md`.
- Behavior:
  - `answer_quality_autofix` live enqueue now requires the dedicated `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED=true` flag in addition to the fixed-task global switch and allowlists;
  - when the dedicated flag is absent or false, passive cases can still be collected but DataMax records a `answer_quality_autofix_disabled` preflight rejection instead of creating a Codex Host task;
  - the hard customer-facing answer-quality gate remains disabled and no normal answer is suppressed by this path.
- Local verification:
  - `cargo fmt --check -p platform-api -p codex-host-agent` passed.
  - `cargo test -p platform-api answer_quality_autofix --lib` passed, 14 tests including the dedicated opt-in guard.
  - `cargo test -p codex-host-agent answer_quality --lib` passed, 3 tests.
  - `cargo test -p platform-api assistant_run_answer_quality_gate --lib` passed, 14 tests.
  - `powershell -ExecutionPolicy Bypass -File .\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary` passed.
  - `powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local` passed.
  - `cargo check -p platform-api -p codex-host-agent` passed.
- Smoke receipts:
  - Fixed-task JSON: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T053911Z.json`.
  - Fixed-task Markdown: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T053911Z.md`.
  - Quality-gate JSON: `target/document-quality-smoke/document-quality-smoke-20260606T053851Z-5476.json`.
  - Quality-gate Markdown: `target/document-quality-smoke/document-quality-smoke-20260606T053851Z-5476.md`.
- Remaining:
  - keep live enqueue disabled unless the operator explicitly sets all required flags and allowlists.

### 2026-06-06 8-Server Answer Quality Autofix Safe-Disabled Deployment

- Commit deployed:
  - `3394b0a5f60a`.
- Deployment:
  - 8 server fast-forwarded from `4a19ab402` to `3394b0a5f60a`;
  - `cargo build --release -p platform-api` passed;
  - `aiv3-platform-api.service` was restarted;
  - follow-up service probe returned `ActiveState=active`, `SubState=running`, and a live process id.
- Sanitized runtime audit:
  - `ASSISTANT_RUN_ANSWER_QUALITY_GATE_ENABLED`: unset;
  - `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED`: unset;
  - `CODEX_HOST_TASK_ENABLED`: enabled;
  - `CODEX_HOST_TASK_ALLOWLIST` contains `answer_quality_autofix`: no;
  - `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES` contains `answer_quality_autofix`: no.
- Safety result:
  - customer-facing hard answer gate remains off;
  - `answer_quality_autofix` live Codex task creation is impossible under current 8-server flags because the dedicated flag and both allowlist entries are absent;
  - passive local/regression coverage remains the release gate until the operator explicitly opts into live enqueue.
- Server state:
  - repo is on `main...origin/main`;
  - existing untracked `mode` file remains untouched.

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

### 2026-06-06 Controlled Background Enterprise Memory Batch

- 8-server head before the batch:
  - `8faa1b797b58`.
  - `document-fingerprint-backfill` and `document-enrichment-worker` release binaries were present.
- Reviewed dry-run:
  - default recent-20 command shape: `./target/release/document-fingerprint-backfill --limit 20 --dry-run --pretty`;
  - result: `candidate_count=20`, `would_record_count=0`, `skipped_count=20`;
  - interpretation: recent candidates were external objects without local files, so they were not suitable for a production non-dry-run backfill.
- Scope narrowing:
  - database object-key type audit showed `absolute=196` missing fingerprints and `external=2407` missing fingerprints;
  - a single absolute-path document was selected by `--document-id`;
  - single-document dry run result: `candidate_count=1`, `would_record_count=1`, `recorded_count=0`, `duplicate_count=0`, `skipped_count=0`.
- Limited non-dry-run:
  - command shape: `./target/release/document-fingerprint-backfill --document-id <reviewed_document_uuid> --pretty`;
  - result: `candidate_count=1`, `recorded_count=1`, `duplicate_count=0`, `skipped_count=0`;
  - final fingerprint state: `dedup_state=canonical`;
  - rollback boundary: do not delete records; disable enrichment flags and stop the worker if rollout needs to pause.
- Single enrichment run:
  - enqueued one idempotent `fact_index_v2` run for the same document fingerprint;
  - processed with `DOCUMENT_ENRICHMENT_WORKER_ONCE=true`, `DOCUMENT_ENRICHMENT_WORKER_MAX_RUNS=1`, and `DOCUMENT_ENRICHMENT_WORKER_KIND=fact_index_v2`;
  - result: `status=succeeded`, `attempt_count=1`, `fact_count=56`;
  - post-run checks: `document_facts=56`, dataset snapshot row exists, `source_fact_count=210`, `scanned_document_count=6`.
- Local regressions:
  - `powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local` passed.
  - `cargo test -p storage document_canonical_enrichment --lib` passed.
  - `cargo test -p retrieval-worker --bin document-enrichment-worker` passed, 8 tests.
  - `cargo test -p platform-api canonical_duplicate_read_through_reuses_chunks_evidence_and_facts --lib` passed.
  - `cargo test -p platform-api database_aggregate_heuristics --lib` passed, 6 tests.
  - `cargo test -p platform-api assistant_run_deterministic_aggregate_intent_covers_customer_smoke_domains --lib` passed.
  - `cargo test -p platform-api planning_catalog_prefers_aggregate_fact_supply_without_fact_rows --lib` passed.
- Remaining boundary:
  - long-running production enrichment is still not enabled;
  - full existing-document fingerprint backfill is still not enabled;
  - the next rollout should either enable post-ingest enrichment only for newly parsed documents or run another reviewed, low-volume existing-document batch.

### 2026-06-06 Post-Ingest Enrichment Kind Allowlist

- Files changed:
  - `crates/retrieval-worker/src/main.rs`;
  - `docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md`;
  - `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md`;
  - `docs/validation/datamax-main-gap-closure.md`.
- Behavior:
  - post-ingest enrichment enqueue now supports `DOCUMENT_ENRICHMENT_KINDS`, a comma-separated rollout list;
  - accepted entries include versioned names and plan aliases, for example `fact_index_v2`, `table_structure`, `procedure_steps`, `resume_profile`, and `spreadsheet_metrics`;
  - duplicate entries are de-duplicated in input order;
  - unknown entries are ignored, and a configured list with no valid kinds records `no_enabled_enrichment_kinds` instead of enqueueing unexpected work;
  - the default remains the existing full deterministic kind set when `DOCUMENT_ENRICHMENT_KINDS` is absent.
- Local verification:
  - `cargo fmt --check -p retrieval-worker` passed.
  - `cargo test -p retrieval-worker --bin retrieval-worker document_enrichment_kinds` passed, 2 tests.
  - `cargo test -p retrieval-worker --bin document-enrichment-worker` passed, 8 tests.
- Remaining:
  - completed by the 8-server tiny post-ingest upload/enrichment smoke below.

### 2026-06-06 8-Server Post-Ingest Enrichment Rollout

- Commit deployed:
  - `3171fbb5e47d`.
- Deployment:
  - 8 server fast-forwarded from `be89395b3` to `3171fbb5e47d`;
  - `CC=clang CXX=clang++ cargo build --release -p retrieval-worker` passed;
  - `aiv3-retrieval-worker.service` was restarted and returned `active`;
  - repo remains `main...origin/main`; existing untracked `mode` remains untouched.
- Runtime configuration:
  - retrieval-worker drop-in `/etc/systemd/system/aiv3-retrieval-worker.service.d/30-document-enrichment.conf`;
  - `DOCUMENT_ENRICHMENT_ENABLED=true`;
  - `DOCUMENT_ENRICHMENT_KINDS=fact_index_v2,table_structure_v1,procedure_steps_v1,resume_profile_v1,spreadsheet_metrics_v1`;
  - `DOCUMENT_ENRICHMENT_DEFAULT_PRIORITY=500`;
  - `DOCUMENT_ENRICHMENT_MAX_ATTEMPTS=2`.
- Worker service:
  - installed `/etc/systemd/system/aiv3-document-enrichment-worker.service`;
  - enabled and started;
  - `ActiveState=active`, `SubState=running`;
  - process uses `Nice=10`, `CPUWeight=25`, `IOWeight=25`;
  - `DOCUMENT_ENRICHMENT_WORKER_DATABASE_MAX_CONNECTIONS=2`;
  - `DOCUMENT_ENRICHMENT_WORKER_POLL_INTERVAL_MS=15000`;
  - `DOCUMENT_ENRICHMENT_WORKER_ERROR_BACKOFF_SECONDS=300`.
- Smoke/audit:
  - one-shot `document-enrichment-worker` startup succeeded before enabling the continuous service;
  - journal confirms continuous polling started with 15-second interval;
  - journal failure count for `document enrichment run failed` in the checked window was `0`;
  - queue audit showed only the prior `succeeded|fact_index_v2|1` run and no pending/running backlog.
- Safety boundary:
  - no full existing-document fingerprint backfill was run;
  - this rollout affects newly parsed documents that have fingerprints and pass through the normal post-ingest cleanup path;
  - the reviewed tiny upload/new-parse smoke below proves the configured post-ingest path; full existing-document backfill remains disabled.

### 2026-06-06 8-Server Tiny Post-Ingest Upload/Enrichment Smoke

- Scope:
  - target: `8服务器` local API on port `3000`;
  - purpose: prove a newly uploaded and indexed document triggers post-ingest enrichment enqueue/consume in the live 8-server runtime;
  - raw document content, credentials, bearer tokens, database URLs, and full logs were not printed.
- Dataset/document:
  - isolated public smoke dataset key: `codex-post-ingest-enrichment-smoke-public-20260606t060441z`;
  - dataset id: `dd060e95-e96d-4c4a-a4b6-7ef40fec5834`;
  - document id: `7d42ec09-25d0-48fd-af87-78e187bcfa9e`;
  - upload ingest workflow execution id: `b3982ac3-cf21-4f02-b193-544c06d185a4`.
- Ingest/indexing result:
  - document reached `indexed`;
  - `retrieval_indexed=true`;
  - content fingerprint was recorded;
  - recorded size: `328` bytes;
  - workflow tasks succeeded for `ingest_uploaded_document`, `cleanup_document_facts`, and `index_retrieval_artifacts`.
- Enrichment result:
  - `fact_index_v2`: `succeeded`, attempt `1`;
  - `table_structure_v1`: `succeeded`, attempt `1`;
  - `procedure_steps_v1`: `succeeded`, attempt `1`;
  - `resume_profile_v1`: `succeeded`, attempt `1`;
  - `spreadsheet_metrics_v1`: `succeeded`, attempt `1`.
- Interpretation:
  - the live retrieval-worker post-ingest path enqueued only the configured deterministic enrichment kinds;
  - the low-priority `aiv3-document-enrichment-worker.service` consumed all five runs successfully;
  - no unexpected enrichment kinds were observed for the smoke document;
  - the smoke did not run or require full existing-document backfill.
- Notes:
  - an initial local-thread-only dataset attempt could create an isolated dataset but could not register a document through the current public register route because that route's dataset visibility lookup does not pass local-thread scope; the public isolated smoke above was used instead.
  - this local-thread visibility edge is not part of the third-party public contract and is left as a possible future internal cleanup item.

### 2026-06-06 Local Operator Observability Summary

- Files changed:
  - `apps/web/app/lib/external-integrations.js`;
  - `apps/web/app/lib/external-integrations.test.mjs`;
  - `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`;
  - `apps/web/app/layout.js`;
  - `apps/web/app/components/Sidebar.js`;
  - `apps/web/app/globals.css`.
- Behavior:
  - adds a compact `运营总览` panel to `/external-integrations`;
  - summarizes ordinary chat, model lane, workflow backlog, report/static-page jobs, template/artifact state, data ingestion, document enrichment, and low-quality recovery;
  - uses `/api/v3/external/codex-executor-tasks/queue-stats` for sanitized queue summaries;
  - uses `/api/v3/model-gateway/status` for model-lane status when the current session is allowed;
  - keeps Codex task runtime inspect, conversation timeline/debug payloads, and database source detail panels lazy-loaded;
  - does not introduce or expose credentials, raw prompts, raw replies, raw document text, raw source rows, or database URLs.
  - updates visible app title copy from legacy V3 wording to DataMax where this slice touched the web shell.
- Verification:
  - `node --test app/lib/external-integrations.test.mjs` from `apps/web` passed, 27 tests;
  - `npm --prefix apps/web run build` passed;
  - `GET http://127.0.0.1:3100/external-integrations` returned HTTP `200` during local dev-server smoke;
  - the build emitted the existing Next.js middleware/proxy deprecation warning and NFT trace warning from `next.config.js`/local upload route; no new build failure was introduced.
- Remaining boundary:
  - 8-server page and queue-summary deployment smoke passed;
  - production operator smoke should still verify model-gateway status with a main-system operator session/cookie;
  - low-quality recovery remains passive and must not suppress normal customer answers.

### 2026-06-06 8-Server Operator Observability Deployment Receipt

- Commit:
  - `2193e0cd5248 Expand DataMax operator observability`.
- Deployment command shape:
  - `git pull --ff-only`;
  - `npm --prefix apps/web run build`;
  - `sudo systemctl restart aiv3-web.service`.
- Result:
  - 8 server fast-forwarded from `8faa1b797` to `2193e0cd5248`;
  - web build succeeded;
  - `aiv3-web.service` active;
  - `aiv3-platform-api.service` active;
  - repository status remained `## main...origin/main` with the pre-existing untracked `mode` left untouched.
- Smoke:
  - `GET http://127.0.0.1:3100/external-integrations` returned `200`;
  - `GET https://v3.elepcloud.com/external-integrations` returned `200`;
  - both local and public page HTML contained `DataMax` and `运营总览`;
  - platform queue stats returned `200` with `execution_count=197`, `task_count=392`, `queue_count=6`;
  - web queue-stats proxy returned `200` with the configured observability cookie and a limited response of `execution_count=5`, `task_count=5`, `queue_count=1`;
  - unauthenticated model-gateway status returned `401 auth_session_required`, which confirms the protected boundary but leaves authenticated operator status smoke pending.
- Build warnings:
  - existing Next.js middleware/proxy deprecation warning;
  - existing Turbopack NFT trace warning from `next.config.js` and the local document upload route.

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

### 2026-06-06 8-Server Retrieval Evidence Idempotency Deployment

- Commit:
  - `8fd0a1d69df0`.
- Deployment command shape:
  - `git pull --ff-only`;
  - `CC=clang CXX=clang++ cargo build --release -p platform-api -p retrieval-worker`;
  - restart `aiv3-platform-api.service` and `aiv3-retrieval-worker.service`.
- Result:
  - server fast-forwarded from `688643f780f3` to `8fd0a1d69df0`;
  - release build succeeded;
  - `aiv3-platform-api.service` active;
  - `aiv3-retrieval-worker.service` active;
  - existing untracked server file `mode` was observed and not touched.
- Guarded operator sync retry:
  - AssistantRun `de6755e0-b490-4665-901b-8847b7a0081b`;
  - plan id `staging-plan-ca9e6b0e-7f16-4fc0-b979-e1ad63afba06`;
  - source id `hy-sql-traffic-area`;
  - confirm returned HTTP 200;
  - created dataset false because dataset `ac7bb786-3ffb-40e2-bade-9f70d5fb4764` already existed from the previous confirmation;
  - sync returned HTTP 202;
  - sync run id `0f75e5ef-130a-4ba8-a4c6-efe880db5ce2`;
  - deduplicated false;
  - production write allowed false.
- AssistantRun event result:
  - latest terminal event `assistant_run.data_ingestion_staging_sync_updated`;
  - workflow stage `completed`;
  - workflow status `succeeded`;
  - previous duplicate-key failure did not recur.
- DataMax dataset evidence state:
  - dataset id `ac7bb786-3ffb-40e2-bade-9f70d5fb4764`;
  - lifecycle `draft`;
  - current documents 384;
  - current chunks 384;
  - current retrieval evidence rows 384;
  - all current documents are indexed.
- External sync run state:
  - `external_sync_runs.status=succeeded`;
  - `sync_kind=full`;
  - `failure_kind` empty;
  - counts recorded `row_count=577`, `documents_ingested=577`, `chunks_ingested=577`, `chunks_indexed=577`, `retrieval_evidences_indexed=577`;
  - `failed_row_count=0`.
- Follow-up audit:
  - current dataset has 384 unique document/chunk/evidence rows while sync counts recorded 577 processed/indexed rows;
  - current table distribution is `bi_contract_warning=6`, `bi_oa_zulinhetong=100`, `bi_oa_zulinhetonggudingzujin=100`, `bi_oa_zulinhetongtichengzujin=100`, `bi_rentsales_detail=1`, `nwstore=77`;
  - this does not block the duplicate-key fix, but should be audited before claiming full source materialization count accuracy.
- Safety:
  - no third-party public URL/auth/request/response field was changed;
  - no bearer token, session token, database URL, source credential, or raw source row was recorded.

### 2026-06-06 Data-Ingestion Count Audit And Local Worker Fix

- Scope:
  - 8-server read-only audit of sync run `0f75e5ef-130a-4ba8-a4c6-efe880db5ce2`;
  - dataset `ac7bb786-3ffb-40e2-bade-9f70d5fb4764`;
  - source `hy-sql-traffic-area`.
- Sanitized audit result:
  - sync run `row_count=577`;
  - current unique documents 384;
  - current unique chunks 384;
  - current retrieval evidence rows for workflow execution `e3f1dfda-504d-4b61-a2f8-71163c698581`: 384;
  - per-table current materialization: `bi_contract_warning=6`, `bi_oa_zulinhetong=100`, `bi_oa_zulinhetonggudingzujin=100`, `bi_oa_zulinhetongtichengzujin=100`, `bi_rentsales_detail=1`, `nwstore=77`.
- Root cause:
  - the worker counted source rows and repeated processing attempts as `documents_ingested`, `chunks_ingested`, `chunks_indexed`, and `retrieval_evidences_indexed`;
  - ingest passed duplicate `document_ids` to retrieval when multiple source rows mapped to the same `document_external_id`;
  - retrieval-evidence idempotency prevented duplicate table rows, so the actual evidence table was correct but the sync-run count was misleading;
  - current MySQL identity mappings collapse many rows for `bi_contract_warning` and `bi_rentsales_detail`; these mappings need a separate business review before row-level completeness is claimed.
- Local code fix:
  - `crates/ingest-worker/src/main.rs` now keeps `row_count` as source rows, records `source_rows_ingested`, attempted counts, unique materialized document/chunk counts, and `collapsed_duplicate_row_count`;
  - `documents_ingested` and `chunks_ingested` now use unique materialized counts from the ingest stage;
  - `document_ids` passed to the retrieval stage are unique;
  - `crates/retrieval-worker/src/main.rs` also de-duplicates incoming document ids defensively and records `documents_indexed`, `unique_chunks_indexed`, and `retrieval_evidences_materialized`.
- Local verification:
  - `cargo fmt --check -p ingest-worker -p retrieval-worker` passed;
  - `cargo test -p ingest-worker --bin ingest-worker external_source_ingest_table_counts -- --nocapture` passed, 1 test;
  - `cargo test -p retrieval-worker --bin retrieval-worker external_index_document_ids -- --nocapture` passed, 2 tests;
  - `cargo check -p ingest-worker -p retrieval-worker` passed.
- Pending:
  - review identity mappings for `bi_contract_warning` and `bi_rentsales_detail` if the business wants row-level rather than entity-level materialization.
- Safety:
  - no raw source rows, credentials, database URL, bearer token, or session token were recorded;
  - no third-party public URL, auth, required request field, or existing response field was changed.

### 2026-06-06 8-Server Data-Ingestion Count Fix Deployment

- Commit:
  - `8c72144aa5f9` (`Fix external source materialization counts`).
- Deployment:
  - `/srv/aiv3/repo` fast-forwarded from `74ea7c7ac` to `8c72144aa5f9`;
  - release build passed for `ingest-worker` and `retrieval-worker`;
  - `aiv3-ingest-worker.service` active;
  - `aiv3-retrieval-worker.service` active;
  - existing untracked server file `mode` was observed and not touched.
- Re-sync receipt:
  - trigger path: source-level ExternalSourceSync into source-owned smoke dataset, not the private operator-confirmed staging dataset;
  - source `hy-sql-traffic-area`;
  - sync run `e9da6483-5705-416e-bdc2-a1cc219f6566`;
  - workflow execution `5ca412ad-04e9-47f0-a0a0-28ad03fe85e8`;
  - target dataset `cd024465-358e-458c-961d-a8894f2358c5`;
  - terminal status `succeeded`;
  - workflow stage `completed`;
  - failure kind empty.
- Corrected sync counts:
  - `row_count=577`;
  - `source_rows_ingested=577`;
  - `documents_ingested=384`;
  - `chunks_ingested=384`;
  - `chunks_indexed=384`;
  - `retrieval_evidences_indexed=384`;
  - `unique_documents_materialized=384`;
  - `unique_chunks_materialized=384`;
  - `unique_chunks_indexed=384`;
  - `retrieval_evidences_materialized=384`;
  - `documents_ingested_attempted=577`;
  - `chunks_ingested_attempted=577`;
  - `collapsed_duplicate_row_count=193`.
- Per-table collapsed-row signal:
  - `bi_contract_warning`: 100 source rows, 6 unique materialized documents, collapsed duplicate rows 94;
  - `bi_rentsales_detail`: 100 source rows, 1 unique materialized document, collapsed duplicate rows 99;
  - `bi_oa_zulinhetong`, `bi_oa_zulinhetonggudingzujin`, `bi_oa_zulinhetongtichengzujin`, and `nwstore`: no collapse under the current smoke batch.
- DataMax table cross-check:
  - `bi_contract_warning`: documents 6, chunks 6, evidence 6;
  - `bi_oa_zulinhetong`: documents 100, chunks 100, evidence 100;
  - `bi_oa_zulinhetonggudingzujin`: documents 100, chunks 100, evidence 100;
  - `bi_oa_zulinhetongtichengzujin`: documents 100, chunks 100, evidence 100;
  - `bi_rentsales_detail`: documents 1, chunks 1, evidence 1;
  - `nwstore`: documents 77, chunks 77, evidence 77.
- Live source smoke:
  - command: `DATA_INGESTION_LIVE_SMOKE_DATABASE_URL="$PLATFORM_DATABASE_URL" DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY=hy-sql-traffic-area DATA_INGESTION_LIVE_SMOKE_API_BASE=http://127.0.0.1:3000 bash scripts/run-data-ingestion-staging-live-smoke.sh`;
  - JSON receipt `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T045752Z.json`;
  - Markdown receipt `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T045752Z.md`;
  - result passed;
  - question/report ready yes;
  - basis dataset `external-source-hy-sql-traffic-area-dataset-hy-sql-traffic-area-count-fix-smoke`;
  - warning retained: default dataset has no indexed database-source documents yet.
- Safety:
  - no raw source rows, credentials, database URL, bearer token, or session token were recorded;
  - no third-party public URL, auth, required request field, or existing response field was changed.

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
