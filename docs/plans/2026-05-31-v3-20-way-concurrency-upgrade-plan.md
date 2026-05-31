# V3 20-Way Concurrency Upgrade Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make both V3 main site and third-party external channels reliably support 20 concurrent ordinary conversations, while allowing heavy jobs such as Image2-to-HTML/static-page generation to run at 5-way concurrency, or 2-way concurrency when Cloudflare Codex fallback is used.

**Architecture:** Keep public third-party URLs, authentication, and request/response fields stable. Add internal concurrency control, per-conversation ordering, faster model fallback, worker scaling, and queue observability around the existing `platform-api`, model gateway profiles, `workflow_tasks`, and worker binaries. Ordinary chat stays latency-prioritized; heavy jobs are queued and isolated from chat capacity.

**Tech Stack:** Rust/Axum platform API, PostgreSQL/sqlx storage, NATS task wakeups, systemd services on 8 server, Next.js web UI, existing model gateway profile table, Right Code GPT-5.5, MiniMax fallback, Cloudflare Codex/image queue.

---

## Requirements

- Main site ordinary chat: support 20 concurrent active user conversations.
- Third-party ordinary chat: support 20+ concurrent conversations across `conversation_external_id`.
- Heavy operations: support 5 concurrent jobs for Image2/HTML/static-page/data-heavy operations.
- Cloudflare Codex fallback path: support 2 concurrent jobs.
- Do not change third-party public interfaces, URLs, auth, or request/response fields.
- Preserve existing idempotency semantics and improve duplicate safety.
- Keep ordinary chat isolated from document parsing, static-page generation, Image2, data ingestion, and Codex executor work.
- Provide smoke/load tests that prove both main site and third-party concurrency behavior.

## Current Findings

- Third-party `/v1/external/channels/{connection_id}/events` and `/events/stream` are served directly by `platform-api`, not by the single `assistant-run-worker`.
- Third-party ordinary chat already uses the model gateway pool when `LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=active`.
- 8 server currently has Right `gpt-5.5` profile at `max_concurrency=20` and MiniMax fallback at `max_concurrency=6`.
- Main site chat/session path still relies on `chat-session-worker`, which claims one task at a time.
- Worker binaries generally run a single claim/process loop per service.
- `PgStorage::connect` defaults to a 10-connection pool per process, so worker horizontal scaling must be paired with DB pool control.
- External message idempotency is protected by `external_message_events(tenant_id, idempotency_key)`, but the current flow checks idempotency before creating the run and records the event afterward, leaving a narrow concurrent duplicate window.

## Implementation Progress

- Done: external direct reply defaults moved to 20-way-safe values and Right `gpt-5.5` preset added for the `assistant_chat` lane.
- Done: third-party idempotency pre-claim closes the duplicate run window while preserving response shape.
- Done: third-party same-conversation in-flight guard returns processing for overlapping messages and allows different conversations to run in parallel.
- Done: main-site chat-session worker can use the same `assistant_chat` model pool and has bounded worker concurrency via `CHAT_SESSION_WORKER_CONCURRENCY`.
- Done: static-page worker and Codex host agent have bounded heavy concurrency via `STATIC_PAGE_IMAGE2_HTML_CONCURRENCY` and `CODEX_HOST_CLOUDFLARE_CONCURRENCY`.
- Done: database pool caps can be configured globally and per service.
- Done: model gateway operator status now exposes worker concurrency and DB pool caps for the main concurrency bottlenecks.
- Done: third-party 20-way, main-site 20-way, and static-page 5-way smoke scripts are available under `scripts/smoke/`.
- Existing: workflow queue aggregate status is already exposed at `/v1/workflow-tasks/queue-stats` and used by the external integrations UI.
- Done: external-channel runtime status exposes active conversation guard count, recent pending idempotency preclaims, and direct-reply timeout budgets.
- Done: Cloudflare fallback 2-way guard smoke checks Codex host cap and watched queue running counts without enqueuing expensive jobs.
- Remaining: 8-server rollout validation with real credentials/cookies during a deployment window.

## Task 1: Concurrency Contract And Runtime Knobs

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `docs/operations/model-gateway-rollout.md`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add explicit env-driven defaults for external direct reply:
   - `EXTERNAL_CHANNEL_DIRECT_REPLY_ATTEMPT_TIMEOUT_MS=20000`
   - `EXTERNAL_CHANNEL_DIRECT_REPLY_TOTAL_BUDGET_MS=60000`
   - keep code defaults unchanged unless tests require safer defaults.
2. Add a documented operating profile for 20-way third-party chat:
   - Right `max_concurrency=20`
   - Right `rpm_limit=120`
   - MiniMax fallback `max_concurrency=6`
   - MiniMax `rpm_limit=120`
3. Add tests for model gateway attempt timeout reading and fallback sequencing.
4. Verify:
   - `cargo test -p platform-api external_channel_direct_reply`
   - `cargo test -p platform-api model_gateway_active_rpm_limit_falls_back_to_next_profile`

**Acceptance:**

- Right can handle 20 concurrent in-flight ordinary chat attempts.
- RPM does not throttle normal 20-user two-turn bursts.
- Fallback starts within the configured attempt timeout.

## Task 2: Third-Party Idempotency Pre-Claim

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Optional migration: `crates/storage/migrations/00XX_external_message_event_preclaim.sql`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Introduce an internal pre-claim helper for `external_message_events`.
2. On inbound third-party message:
   - insert a row with `assistant_run_id = null`, `direction = 'inbound'`, and the supplied idempotency key before creating the assistant run.
   - if the insert conflicts, return the existing run if available, otherwise return an accepted/processing reply.
3. After run creation, update the pre-claimed row with `assistant_run_id` and final payload summary.
4. Keep the public response shape unchanged.
5. Add a concurrency test that submits the same idempotency key twice concurrently and asserts only one assistant run is created.

**Acceptance:**

- Duplicate requests with the same idempotency key cannot create duplicate runs, even under concurrent retries.

## Task 3: Per-Conversation Ordering For Third-Party Chat

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Optional migration: `crates/storage/migrations/00XX_external_conversation_locks.sql`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add an internal per-conversation in-flight guard keyed by:
   - `tenant_id`
   - `connection_id`
   - `platform`
   - `conversation_external_id`
2. First implementation can use PostgreSQL advisory locks for direct replies.
3. If a second message arrives for the same conversation while one is active:
   - for `/events/stream`, emit a quick processing status.
   - for `/events`, return accepted/processing rather than running two answers out of order.
4. Do not serialize different conversations.
5. Add tests:
   - same conversation serializes or gets processing status.
   - different conversations can run concurrently.

**Acceptance:**

- 20 different conversations run in parallel.
- Same conversation does not produce out-of-order model answers.

## Task 4: Main Site Chat Uses The Same Model Pool

**Files:**
- Modify: `crates/chat-session-worker/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/llm-gateway/src/lib.rs` if shared helpers are needed
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add a shared model-pool resolution helper for main-site chat/session, equivalent to external `assistant_chat`.
2. Route chat-session turns through model gateway profiles when enabled.
3. Preserve legacy `CHAT_SESSION_RUNTIME_*` as fallback if no enabled profile exists.
4. Add worker-level concurrency control:
   - `CHAT_SESSION_WORKER_CONCURRENCY=20`
   - default remains current single loop until explicitly configured.
5. Implement bounded task spawning inside `chat-session-worker` using `tokio::Semaphore`.
6. Add tests for:
   - model pool profile selection.
   - legacy fallback.
   - bounded concurrent task processing.

**Acceptance:**

- Main site ordinary chat can process 20 concurrent turns without waiting for a single worker loop.
- Same model profile behavior and observability apply to main site and third-party chat.

## Task 5: Heavy Job Queue Isolation And Concurrency

**Files:**
- Modify: `crates/static-page-worker/src/main.rs`
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/plans/2026-05-30-static-page-image-pipeline-optimization-plan.md`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Define logical heavy queues:
   - `static_page_image_preview`
   - `static_page_render`
   - `static_page_publish`
   - `codex_host_cloudflare_fallback`
2. Add env concurrency knobs:
   - `STATIC_PAGE_IMAGE2_HTML_CONCURRENCY=5`
   - `STATIC_PAGE_RENDER_CONCURRENCY=5`
   - `CODEX_HOST_CLOUDFLARE_CONCURRENCY=2`
3. Keep heavy tasks off ordinary chat execution.
4. Ensure static-page worker can process up to 5 non-Cloudflare heavy tasks concurrently.
5. Ensure Cloudflare Codex fallback path is capped at 2.
6. Add queue tests for concurrency caps and non-starvation of chat tasks.

**Acceptance:**

- 5 heavy Image2/HTML jobs can run simultaneously.
- Cloudflare fallback never exceeds 2 concurrent jobs.
- Heavy jobs do not consume ordinary chat model permits.

## Task 6: DB Pool And Worker Scaling Safety

**Files:**
- Modify: `crates/storage/src/lib.rs`
- Modify worker mains:
  - `crates/platform-api/src/main.rs`
  - `crates/chat-session-worker/src/main.rs`
  - `crates/assistant-run-worker/src/main.rs`
  - `crates/static-page-worker/src/main.rs`
  - `crates/codex-host-agent/src/main.rs`
  - other worker `main.rs` files as needed
- Test: `crates/storage/src/lib.rs`

**Steps:**

1. Add `PLATFORM_DATABASE_MAX_CONNECTIONS` defaulting to current `10`.
2. Add per-service overrides:
   - `PLATFORM_API_DATABASE_MAX_CONNECTIONS`
   - `CHAT_SESSION_DATABASE_MAX_CONNECTIONS`
   - `STATIC_PAGE_DATABASE_MAX_CONNECTIONS`
   - `CODEX_HOST_DATABASE_MAX_CONNECTIONS`
3. Keep 8 server total planned DB connections below Postgres `max_connections=100`.
4. Recommended initial 8 server config:
   - platform-api: 20
   - chat-session-worker: 10
   - static-page-worker: 8
   - codex-host-agent: 4
   - other workers: 3-5 each
5. Add tests for env parsing.

**Acceptance:**

- Worker scaling does not accidentally exhaust Postgres connections.

## Task 7: Concurrency Observability Page/API

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/components/ModelPoolPanel.js`
- Modify: `apps/web/app/lib/model-gateway.js`
- Test: `apps/web/app/lib/model-gateway.test.mjs`

**Steps:**

1. Extend model gateway status with:
   - current profile active/queued/max concurrency.
   - p50/p95 latency from recent events where available.
   - timeout/rate-limit/fallback counts.
2. Extend workflow queue status with:
   - queued/running/claimed counts.
   - oldest waiting age.
   - logical queue name.
3. Add external-channel concurrency counters:
   - active direct replies.
   - per-conversation in-flight count.
   - duplicate idempotency pre-claim count.
4. Render a compact operator panel in the existing model pool/observability UI.

**Acceptance:**

- Operators can see whether bottleneck is model, DB, worker queue, or Cloudflare fallback.

## Task 8: 20-Way Smoke And Load Harness

**Files:**
- Create: `scripts/smoke/external-channel-20way.mjs`
- Create: `scripts/smoke/main-chat-20way.mjs`
- Create: `scripts/smoke/heavy-static-page-5way.mjs`
- Modify: `package.json` or existing smoke runner if present

**Steps:**

1. Third-party 20-way smoke:
   - create 20 unique `conversation_external_id`.
   - send ordinary document-backed or general questions through `/events/stream`.
   - assert all responses complete or accepted within SLA.
2. Main site 20-way smoke:
   - create 20 local chat/session turns.
   - assert completion and no serial backlog.
3. Heavy 5-way smoke:
   - enqueue 5 static-page/Image2/HTML jobs.
   - assert 5 can be running/claimed.
4. Cloudflare fallback smoke:
   - enqueue 3 fallback jobs.
   - assert only 2 run concurrently.
5. Record:
   - p50/p95 latency.
   - timeout count.
   - fallback count.
   - DB connection high-water mark.

**Acceptance:**

- Third-party ordinary chat: 20 concurrent conversations complete without 429 from V3 local limiter.
- Main site ordinary chat: 20 concurrent turns complete without single-worker serialization.
- Heavy jobs: 5 concurrent local heavy jobs, 2 concurrent Cloudflare fallback jobs.

## Task 9: 8 Server Rollout

**Files:**
- Modify server env only after code/tests pass:
  - `/etc/aiv3/aiv3.env`
  - `/etc/aiv3/minimax.env`
- No public third-party API contract changes.

**Steps:**

1. Deploy code to 8 server.
2. Set conservative env:
   - `LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=active`
   - Right `max_concurrency=20`, `rpm_limit=120`
   - MiniMax `max_concurrency=6`, `rpm_limit=120`
   - `EXTERNAL_CHANNEL_DIRECT_REPLY_ATTEMPT_TIMEOUT_MS=20000`
   - `EXTERNAL_CHANNEL_DIRECT_REPLY_TOTAL_BUDGET_MS=60000`
   - `CHAT_SESSION_WORKER_CONCURRENCY=20`
   - `STATIC_PAGE_IMAGE2_HTML_CONCURRENCY=5`
   - `CODEX_HOST_CLOUDFLARE_CONCURRENCY=2`
3. Restart affected services.
4. Run smoke scripts.
5. Watch observability for 30 minutes.

**Acceptance:**

- No third-party re-integration required.
- Existing requests remain compatible.
- 20-way ordinary chat and 5-way heavy job smoke pass on 8 server.

## Recommended Implementation Order

1. Task 1: runtime knobs and Right RPM.
2. Task 8 partial: third-party 20-way smoke harness, first against current system.
3. Task 2: idempotency pre-claim.
4. Task 3: per-conversation ordering.
5. Task 4: main site chat model-pool/concurrency.
6. Task 6: DB pool safety.
7. Task 5: heavy job queue concurrency.
8. Task 7: observability page/API.
9. Task 9: 8 server rollout and long smoke.

## Rollback Plan

- Set `LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=observe_only` to stop third-party active model-pool routing.
- Restore Right `max_concurrency` and `rpm_limit` to previous values.
- Set `CHAT_SESSION_WORKER_CONCURRENCY=1`.
- Set heavy job concurrency back to `1`.
- Revert only the latest code deployment if schema changes cause unexpected behavior; avoid changing third-party payload contracts.

## Open Questions

- Whether main-site chat should enforce strict per-thread ordering or allow multiple in-flight turns per local thread.
- Whether Right account-level `50` concurrency is shared with other systems outside V3.
- Whether Cloudflare Codex fallback concurrency should be global across tenants or per connection.
- Whether third-party requires synchronous `/events` responses for all ordinary chat, or can standardize on `/events/stream`.
