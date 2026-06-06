# DataMax Main Gap Closure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the remaining major DataMax production gaps after the model-visible capability loop rollout: 20-way concurrency, background document enrichment, deterministic enterprise facts, low-quality answer recovery, confirmed data ingestion, controlled streaming, static-page template operations, and 8-server validation.

**Architecture:** Keep DataMax as the system of record for tenants, datasets, document scopes, parsed evidence, fact snapshots, AssistantRun state, workflow tasks, generated artifacts, and third-party contracts. New work must add internal queues, validation ledgers, background enrichment, and operator views around existing APIs rather than changing public third-party URLs, auth, or request/response fields. Customer-facing answers stay model-authored; platform tools provide scoped evidence, reports, status, and artifact links.

**Tech Stack:** Rust `platform-api`, `storage`, `ingest-worker`, `retrieval-worker`, `static-page-worker`, `codex-host-agent`, PostgreSQL, NATS/workflow tasks, Next.js DataMax web app, existing smoke scripts under `scripts/`, 8-server systemd services, Cloudflare/Image2/Codex fixed-task bridge.

---

## Current Baseline - 2026-06-06

- Local `main` is clean at `b9787a774958`.
- 8 server `/srv/aiv3/repo` is also at `b9787a774958`; `aiv3-platform-api.service`, `aiv3-web.service`, `aiv3-static-page-worker.service`, and `aiv3-document-enrichment-worker.service` are active.
- 8 server still has the pre-existing untracked `mode` file; do not touch it during deploys.
- Queue stats endpoint is reachable on 8 server; latest lightweight check returned `execution_count=198`, `task_count=395`, and `generated_at=2026-06-06T06:41:26Z`.
- Production/demo gates already proven in validation:
  - third-party ordinary chat 20-way;
  - main-site chat 20-way;
  - main-site AssistantRun true SSE streaming;
  - third-party progress/artifact streaming;
  - static-page 5-way;
  - Cloudflare fallback concurrency guard;
  - Xinbai explicit report trigger, one focused link, and export files;
  - data-ingestion staging plan, operator-confirmed sync, idempotent retrieval indexing, and truthful source-row vs unique-materialization counters;
  - newly parsed document enrichment for the five configured deterministic kinds.
- Static-page generation should prefer the local/DataMax path. Cloudflare/Codex remains a bounded fallback path and must not be the only working path.
- Existing detailed plans remain reference material only. This document is the single active execution plan; the top execution sheet below overrides older completed task lists later in the file.

## Non-Negotiable Rules

- Do not change third-party public URLs, authentication, required request fields, or existing response fields unless the operator explicitly approves.
- Additive response fields and docs are allowed only when old clients can ignore them.
- Do not touch 120 server.
- 8 server is the production/demo validation target.
- Do not re-enable a hard customer-facing answer-quality gate. The previous gate blocked too many normal answers.
- Do not make Codex executor mutate datasets, permissions, credentials, deployment config, or public integration contracts.
- Do not put raw credentials, database URLs, API keys, full customer documents, raw provider payloads, or full logs into prompts, events, docs, or artifacts.
- VLM reparse is premium and budget-gated. Use it only after cheaper parse/enrichment recovery fails or an operator explicitly approves.
- Static-page/Image2 high-quality flow remains image-first. Reused accepted templates can return immediately while background refresh continues.
- Third-party temporary document scopes and dataset scopes are authoritative for the conversation.
- DataMax does not depend on model-side memory. Every model call should remain stateless and receive only DataMax-scoped temporary evidence.

## Active Execution Sheet - 2026-06-06

Treat this section as the current executable plan. The longer task bodies below are historical detail and implementation references only.

| Priority | Gap | Current state | Next executable action | Done when |
| --- | --- | --- | --- | --- |
| P0 | G3 low-load static-page template prewarm | Local implementation now creates a customer-invisible draft plus delayed existing `static_page/generate_static_page_image` task. Worker low-load recheck/requeue is implemented and locally tested. Production flag remains unset pending 8-server private smoke. | Deploy changed binaries to 8 server and run private scoped prewarm smoke with the flag enabled only for the reviewed test window. | A private scoped chat can silently queue, consume, publish, and store a reusable template without any customer outbound reply. High-load conditions requeue instead of competing with explicit work. |
| P0 | Release smoke after any P0 change | 8 server is stable at `b9787a774958`. | After G3 implementation, run local tests, deploy changed binaries only, and rerun 8-server smoke for ordinary chat, streaming, explicit report/export, static page, and prewarm no-reply behavior. | Validation doc records commit, service status, smoke receipts, and rollback note. Third-party public URLs/auth/request/response fields remain unchanged. |
| P1 | G5 passive low-quality recovery | Safe-disabled deploy is live. Hard gate remains off. Live `answer_quality_autofix` enqueue is impossible under current flags/allowlists. | Keep disabled by default. If enabled later, enable passive collection plus manual review only, with strict fixed-task write scope and no answer suppression. | Weak-answer cases are classified and visible to operators; no normal answer is blocked; no Codex task can be created unless the dedicated flag and both allowlists are explicitly enabled. |
| P1 | G8 authenticated model-gateway operator smoke | Operator summary page and queue summary work. Model-gateway status correctly returns `401` without a main-system operator session. | Obtain or use an existing legitimate operator session/cookie; run status/profile/test smoke without creating an auth bypass or printing secrets. | Operator page can show model lane health and profile state for a signed-in operator; unauthenticated access still returns `401`. |
| P1 | G6 data-source row identity review | Sync counters are truthful. `bi_contract_warning` and `bi_rentsales_detail` currently collapse many source rows into fewer DataMax documents because of configured identity columns. | Review table identity mappings with business intent. If row-level materialization is required, change identity mapping in a staging-only slice and rerun guarded sync. | Operators can distinguish source rows, unique documents, chunks, evidence, and collapsed rows; Xinbai database-derived reports are not misleading about row-level completeness. |
| P2 | Existing-document enrichment backfill | New documents enrich correctly. Full historical backfill remains disabled. | Keep disabled unless a reviewed low-volume batch is requested. Start with fingerprint dry-run, one dataset, one enrichment kind, and queue/backlog proof. | Historical docs can be enriched without duplicate counting, runaway cost, or upload/chat degradation. |
| P2 | Template library hygiene | Xinbai modular monthly report is the default accepted template. Older report artifacts/templates exist historically. | Keep only accepted templates in normal matching; clean old generated artifacts only when explicitly requested and using the local backup/delete policy. | Same dataset/default_prompt/focus requests reuse the accepted template; old artifacts do not pollute matching or customer-visible links. |

### Immediate Execution Order

1. Implement G3 real prewarm execution path and low-load requeue.
2. Run local G3 tests and `cargo check` for changed crates.
3. Deploy only changed services to 8 server; keep `mode` untouched.
4. Run 8-server prewarm smoke with `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=true` only for the private test window, then decide whether to leave enabled.
5. Rerun release gates: third-party ordinary Q&A, main-site streaming, explicit Xinbai report/export, static-page 5-way, and document-quality regression.
6. Record results in `docs/validation/datamax-main-gap-closure.md`, commit, push, and deploy only after the validation receipt is complete.
7. Move to P1: passive low-quality collection decision, authenticated model-gateway operator smoke, and data-source identity review.

### Task X1: Implement Real Low-Load Static-Page Template Prewarm

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/static-page-worker/src/main.rs`
- Modify if needed: `crates/storage/src/lib.rs`
- Add or modify smoke if needed: `scripts/smoke/static-page-template-prewarm.mjs`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Write failing tests for no-orphan prewarm**

Add platform-api tests proving that `maybe_enqueue_external_channel_static_page_template_prewarm` creates work that an existing worker can consume.

Expected before implementation:

- test fails because prewarm currently targets `static_page_template_prewarm/prewarm_static_page_template`, which has no deployed consumer.

**Step 2: Create a customer-invisible draft and delayed image job**

Implement one of these equivalent approaches:

- preferred: create a `StaticPageDraft` with `source_refs.prewarm.customer_visible=false`, `source_refs.prewarm.key=<prewarm_key>`, and then enqueue the existing `static_page/generate_static_page_image` workflow task with `available_at=now+STATIC_PAGE_TEMPLATE_PREWARM_DELAY_MINUTES`;
- acceptable alternative: add a real service/consumer for `static_page_template_prewarm` that immediately hands off to the same existing static-page image/render pipeline.

Required behavior:

- no third-party public request/response fields change;
- no customer-visible message or outbound reply is sent;
- no duplicate prewarm is queued for overlapping dataset scope plus same `default_prompt`;
- accepted template overlap still skips prewarm;
- prewarm artifacts can become accepted/reusable templates only after successful publish.

**Step 3: Add low-load recheck in `static-page-worker`**

Before expensive Image2/static-page visual work for a prewarm job:

- load the related draft/job;
- detect `source_refs.prewarm.customer_visible=false`;
- inspect workflow pressure for explicit `static_page` and `codex_host` work;
- if pressure is above threshold, requeue the task with a non-consuming delay and append a compact internal event;
- if low load, continue through the normal Image2 preview and auto-publish path.

Use env flags with safe defaults:

- `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=false`;
- `STATIC_PAGE_TEMPLATE_PREWARM_DELAY_MINUTES=30`;
- `STATIC_PAGE_TEMPLATE_PREWARM_RECHECK_DELAY_SECONDS=300`;
- `STATIC_PAGE_TEMPLATE_PREWARM_MAX_ACTIVE_TASKS=0`.

**Step 4: Verify locally**

Run:

```powershell
cargo fmt --check -p platform-api -p static-page-worker
cargo test -p platform-api static_page_template_prewarm --lib
cargo test -p platform-api external_channel_static_page_publish --lib
cargo test -p static-page-worker --bin static-page-worker prewarm
cargo check -p platform-api -p static-page-worker
git diff --check
```

Expected:

- prewarm tests prove a consumable static-page task exists;
- publish tests prove no customer dispatch for silent prewarm;
- worker tests prove high-load requeue and low-load continue.

**Step 5: 8-server private smoke**

Deploy changed crates, then run a private prewarm test window:

```powershell
ssh 8服务器 "set -e; cd /srv/aiv3/repo; git pull --ff-only; CC=clang CXX=clang++ cargo build --release -p platform-api -p static-page-worker; sudo systemctl restart aiv3-platform-api.service aiv3-static-page-worker.service; sleep 5; systemctl is-active aiv3-platform-api.service; systemctl is-active aiv3-static-page-worker.service; git rev-parse --short=12 HEAD"
```

Smoke requirements:

- set `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=true` only for the reviewed test window;
- submit a normal scoped third-party conversation with dataset/document scope and `default_prompt`;
- verify normal answer is unaffected;
- verify `assistant_run.static_page_template_prewarm_queued` or equivalent event;
- verify the task is consumed by the static-page worker;
- verify published artifact/template exists;
- verify no third-party outbound reply is dispatched for the prewarm artifact;
- verify a repeated overlapping request reuses or skips instead of enqueuing another prewarm.

### Task X2: Keep Explicit Report And Static-Page Request Path Stable

**Files:**

- Inspect/modify only if regression appears: `crates/platform-api/src/lib.rs`
- Inspect/modify only if regression appears: `apps/web/app/HomePageClient.js`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Re-run explicit report smoke**

Run:

```powershell
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --timeout-ms 180000
npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
```

Required cases:

- `取高`, `经营状况`, `风险识别`, `销售缺口`, `助推`, and `看看整体经营情况` trigger the Xinbai report path;
- `取高是什么意思？` and `风险识别系统有哪些项目经历？` do not trigger a report;
- report reply exposes one primary link only;
- card title is `新世界百货经营管理月报表`;
- `table-data.csv`, `report.ppt`, and `report.md` are accessible;
- normal answer text is not truncated by report creation.

**Step 2: Fix only if smoke fails**

Do not broaden report triggers globally. Xinbai-specific focus terms may be widened only inside the Xinbai/domain template routing policy.

### Task X3: Decide Passive Low-Quality Recovery Rollout

**Files:**

- Inspect/modify if needed: `crates/platform-api/src/lib.rs`
- Inspect/modify if needed: `crates/codex-host-agent/src/lib.rs`
- Update: `docs/operations/answer-quality-autofix.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Keep the hard gate off**

Confirm on 8 server:

- `ASSISTANT_RUN_ANSWER_QUALITY_GATE_ENABLED` is unset/false;
- `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED` is unset/false unless an explicit operator rollout is approved;
- `answer_quality_autofix` is absent from both live allowlists unless approved.

**Step 2: If enabling passive collection**

Enable only after a recorded decision:

- collect weak-answer signals and classifications;
- do not suppress or replace customer answers;
- route only confirmed system-defect cases to fixed-scope autofix;
- require manual review before code/deploy changes.

Verification:

```powershell
cargo test -p platform-api answer_quality_autofix --lib
cargo test -p platform-api assistant_run_answer_quality_gate --lib
cargo test -p codex-host-agent answer_quality --lib
powershell -ExecutionPolicy Bypass -File .\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary
powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local
```

### Task X4: Run Authenticated Operator Smoke For Model Gateway

**Files:**

- Inspect/modify only if smoke reveals a bug: `apps/web/app/lib/model-gateway.js`
- Inspect/modify only if smoke reveals a bug: `crates/platform-api/src/lib.rs`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Use a real operator session**

Do not add a bypass. Use an existing signed-in operator cookie/session or a documented local-key operator path that already exists.

**Step 2: Verify status and profile operations**

Required checks:

- unauthenticated `/v1/model-gateway/status` still returns `401`;
- authenticated operator status returns active model lane state;
- Right `gpt-5.5` profile remains enabled at concurrency 20;
- MiniMax fallback remains bounded;
- profile test endpoint returns a sanitized result and does not print secrets.

### Task X5: Review Data-Source Identity Mappings

**Files:**

- Inspect: `crates/ingest-worker/src/main.rs`
- Inspect: `crates/retrieval-worker/src/main.rs`
- Inspect stored database-source/table mapping metadata through sanitized SQL only
- Update: `docs/validation/data-ingestion-staging-sync-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Produce a sanitized mapping report**

Report only:

- source id;
- table name;
- configured identity columns;
- source row count;
- unique materialized document count;
- collapsed duplicate row count.

Do not print raw rows, credentials, URLs, or customer table contents.

**Step 2: Decide business semantics**

For `bi_contract_warning` and `bi_rentsales_detail`:

- if entity-level aggregation is intended, keep current mapping and document the collapse;
- if row-level report completeness is required, propose a composite identity mapping and test in staging only.

**Step 3: Verify after any mapping change**

Run:

```powershell
cargo test -p ingest-worker --bin ingest-worker external_source_ingest_table_counts -- --nocapture
cargo test -p retrieval-worker --bin retrieval-worker external_index_document_ids -- --nocapture
cargo check -p ingest-worker -p retrieval-worker
bash scripts/run-data-ingestion-staging-live-smoke.sh
```

### Final Release Gate For This Plan

Before declaring this plan closed, record all of the following in `docs/validation/datamax-main-gap-closure.md`:

- deployed commit;
- changed services and active status;
- 8-server queue snapshot;
- ordinary third-party Q&A smoke;
- main-site streaming smoke;
- explicit report/export smoke;
- static-page 5-way smoke;
- prewarm no-customer-reply smoke;
- document-quality smoke;
- answer-quality safe-disabled or explicitly-enabled state;
- model-gateway authenticated operator smoke, or a recorded reason it remains pending;
- data-source identity mapping decision;
- rollback note.

### P0 Task A: Fix Retrieval-Evidence Idempotency

**Status:** completed locally and deployed to 8 server on commit `8fd0a1d`.

**Files:**

- Inspect: `crates/storage/src/lib.rs`
- Inspect: `crates/retrieval-worker/src/main.rs`
- Inspect: `crates/retrieval-worker/src/lib.rs`
- Inspect: `crates/external-source-worker/src/lib.rs`
- Modify likely: `crates/storage/src/lib.rs`
- Modify tests likely: `crates/storage/src/lib.rs` or `crates/retrieval-worker/src/lib.rs`
- Update docs: `docs/validation/datamax-main-gap-closure.md`
- Update docs: `docs/validation/data-ingestion-staging-sync-smoke.md`

**Step 1: Find the write path**

Run:

```powershell
rg -n "retrieval_evidences|insert.*retrieval|document_chunk_id|execution_id" crates/storage crates/retrieval-worker crates/external-source-worker
```

Expected:

- exact repository method or SQL block that inserts retrieval evidence is identified;
- caller path from external-source sync indexing is identified.

**Step 2: Write a failing regression**

Add a test that runs the same evidence insert twice for the same `execution_id` and `document_chunk_id`.

Expected before implementation:

- test fails with the duplicate-key behavior or proves the current method is not idempotent.

**Step 3: Implement minimal idempotency**

Preferred implementation:

- use `ON CONFLICT (execution_id, document_chunk_id) DO UPDATE` when the duplicate row represents the same indexing attempt and the new values are at least as fresh;
- otherwise use `DO NOTHING` only if the existing row is semantically equivalent and downstream counters are not misreported;
- keep document, chunk, dataset, source, and execution identity unchanged;
- do not hide unrelated source-sync errors.

**Step 4: Verify locally**

Run:

```powershell
cargo fmt --check -p storage -p retrieval-worker -p platform-api
cargo test -p storage retrieval_evidence --lib
cargo test -p retrieval-worker external --lib
cargo test -p platform-api data_ingestion --lib
cargo test -p platform-api external_source_sync --lib
cargo check -p platform-api -p retrieval-worker
bash scripts/run-data-ingestion-staging-sync-smoke.sh
git diff --check
```

If a test filter has no matching tests, record the actual matching filter used in the validation ledger.

**Step 5: Commit**

```powershell
git status --short --branch
git add crates docs/validation
git commit -m "Make retrieval evidence indexing idempotent"
git push
```

### P0 Task B: Deploy And Re-Run 8-Server Data-Ingestion Sync

**Status:** completed for the duplicate-key blocker and count-accuracy fix on 2026-06-06. Retry sync `0f75e5ef-130a-4ba8-a4c6-efe880db5ce2` succeeded after the duplicate-key fix. The follow-up count audit found that the sync processed 577 source rows but the current staging dataset had 384 unique materialized documents/chunks/evidence rows. Worker code now separates source-row counts from unique materialized counts and de-duplicates retrieval indexing input. 8-server re-sync `e9da6483-5705-416e-bdc2-a1cc219f6566` succeeded with the corrected counts.

**Files:**

- Update: `docs/validation/datamax-main-gap-closure.md`
- Update: `docs/validation/data-ingestion-staging-sync-smoke.md`

**Step 1: Deploy changed services**

Build and restart only changed binaries. If only storage/retrieval indexing changed, include `retrieval-worker`; include `platform-api` only if API code changed.

```powershell
ssh 8服务器 "set -e; cd /srv/aiv3/repo; git pull --ff-only; CC=clang CXX=clang++ cargo build --release -p platform-api -p retrieval-worker; sudo systemctl restart aiv3-platform-api.service aiv3-retrieval-worker.service; sleep 5; systemctl is-active aiv3-platform-api.service; systemctl is-active aiv3-retrieval-worker.service; git rev-parse --short=12 HEAD"
```

Expected:

- server fast-forwards to the pushed commit;
- changed services are active;
- untracked server file `mode` is not touched.

**Step 2: Re-run guarded sync**

Use an authenticated operator session and the latest reviewed data-ingestion staging plan. Do not print bearer tokens, session tokens, database URLs, or raw source rows.

Body shape:

```json
{
  "source_id": "hy-sql-traffic-area",
  "sync_kind": "full",
  "force": true,
  "checkpoint": {
    "operator_smoke": "2026-06-06-idempotent-retry"
  }
}
```

Expected:

- confirm remains idempotent;
- sync starts with selected source;
- no duplicate-key failure occurs;
- terminal status is `data_ingestion_staging_sync_completed` or a new truthful non-duplicate failure with sanitized `failure_kind`.

**Step 3: Verify indexed evidence**

Query only DataMax metadata/state:

- source-derived dataset id;
- document count;
- chunk count;
- retrieval evidence count;
- latest sync status;
- sanitized failure kind if failed.

Expected:

- no raw customer table rows are printed;
- no raw credentials are printed;
- evidence count is non-zero if sync completed.

**Step 4: Record receipt**

Update both validation docs with:

- commit;
- services restarted;
- run id;
- plan id;
- source id;
- terminal state;
- counts;
- root cause if not completed.

### P0 Task C: Close Production Smoke

**Files:**

- Update: `docs/validation/datamax-main-gap-closure.md`
- Update if needed: `scripts/smoke/external-channel-20way.mjs`
- Update if needed: `scripts/smoke/main-chat-20way.mjs`
- Update if needed: `scripts/smoke/static-page-5way.mjs`
- Update if needed: `scripts/smoke/cloudflare-fallback-2way.mjs`

Run locally first when possible, then on or against 8 server:

```powershell
cargo fmt --check -p platform-api -p storage -p retrieval-worker -p codex-host-agent
cargo check -p platform-api -p retrieval-worker -p codex-host-agent
cargo test -p platform-api external_channel_model_tool_request --lib
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api dataset_fact_snapshot --lib
cargo test -p platform-api scoped_fact --lib
cargo test -p platform-api answer_quality --lib
cargo test -p codex-host-agent
npm --prefix apps/web run build
npm run build:pure-third-party-guide-html
npm run check:pure-third-party-guide-html
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-capability-routing-smoke.ps1 -BaseUrl https://v3.elepcloud.com
node scripts/smoke/external-channel-20way.mjs --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20
node scripts/smoke/main-chat-20way.mjs --base-url https://v3.elepcloud.com --concurrency 20
node scripts/smoke/static-page-5way.mjs --base-url https://v3.elepcloud.com --concurrency 5
node scripts/smoke/cloudflare-fallback-2way.mjs --base-url https://v3.elepcloud.com
node scripts/smoke/external-channel-streaming-10way.mjs --base-url https://v3.elepcloud.com --connection-id generic-chat-main
bash scripts/run-data-ingestion-staging-sync-smoke.sh
git diff --check
```

If private credentials are unavailable, record that explicitly in `docs/validation/datamax-main-gap-closure.md` and run only read-only/no-token guard probes. Do not claim a production pass from no-token probes.

Expected:

- third-party ordinary Q&A still works under 20-way concurrency;
- main-site ordinary Q&A still works under 20-way concurrency;
- 5 heavy static-page jobs do not starve ordinary chat;
- Cloudflare fallback is bounded at 2-way concurrency;
- report/export smoke returns one primary report link and accessible export files;
- document-quality smoke covers resume project experience, elderly-care procedure answers, attendance date/work-hour formatting, and aggregate-first supply.

**Commit after receipt update:**

```powershell
git add docs/validation scripts
git commit -m "Record DataMax production smoke receipts"
git push
```

### P0 Task D: Close Main-Site Controlled Streaming

**Files:**

- Inspect/modify if needed: `crates/platform-api/src/lib.rs`
- Inspect/modify if needed: `apps/web/app/HomePageClient.js`
- Inspect/modify if needed: `apps/web/app/lib/assistant-run-progress.js`
- Update: `docs/validation/datamax-main-gap-closure.md`
- Reference: `docs/plans/2026-06-01-v3-streaming-session-upgrade-plan.md`

**Step 1: Run local stream regressions**

```powershell
cargo fmt --check -p platform-api
cargo test -p platform-api assistant_run_sse --lib
cargo test -p platform-api assistant_run_continue_sse --lib
cargo test -p platform-api assistant_run_live --lib
cargo test -p platform-api assistant_run_continue --lib
cargo test -p platform-api external_channel_public_stream --lib
npm --prefix apps/web run build
```

Expected:

- new AssistantRun stream emits live deltas behind `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED`;
- continued AssistantRun stream uses the same live-delta path;
- final full-text delta is not duplicated after live deltas.

**Step 2: Run browser/SSE smoke**

Use a private operator/main-site session. Test both:

- new chat run;
- continue existing run.

Expected:

- UI text appears progressively;
- terminal state is consistent across SSE, polling, and final run view;
- third-party stream policy remains progress/artifact-safe.

**Step 3: Record and commit**

```powershell
git add docs/validation
git commit -m "Record DataMax main-site streaming smoke"
git push
```

### P0 Task E: Verify Xinbai Monthly Report Template Operations

**Files:**

- Inspect/modify if needed: `crates/platform-api/src/lib.rs`
- Inspect/modify if needed: `crates/static-page-worker/src`
- Inspect/modify if needed: `apps/web/app`
- Update: `docs/validation/external-report-export-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Validate accepted template**

```powershell
npm run validate:xinbai-report-template
npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
```

Expected:

- the modular monthly report template is still valid;
- mobile layout, focus module ordering, export links, and public assets pass validation.

**Step 2: Run external report/export smoke**

```powershell
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-capability-routing-smoke.ps1 -BaseUrl https://v3.elepcloud.com
```

Required scenarios:

- `取高`;
- `经营状况`;
- `经营健康度`;
- `看看整体经营情况`;
- `风险识别`;
- `看看新街口店经营风险`;
- `销售缺口统计一下，哪些门店需要助推？`;
- non-trigger guard: `取高是什么意思？`;
- non-trigger guard: `风险识别系统有哪些项目经历？`.

Expected:

- report card title is `新世界百货经营管理月报表`;
- one customer-visible link is returned;
- `focus` matches the user concern;
- `table-data.csv`, `report.ppt`, and `report.md` are reachable;
- answer text explains the detected focus, not internal template reuse mechanics.

**Step 3: Decide low-load prewarm**

Enable `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=true` only if:

- P0 concurrency smoke is stable;
- template reuse is proven;
- operator status can show queued/running/prewarm tasks;
- expensive Image2/Codex work remains low priority.

### P1 Task F: Controlled Background Enterprise Memory Batch

**Files:**

- Inspect/modify if needed: `crates/storage/src/lib.rs`
- Inspect/modify if needed: `crates/platform-api/src/bin/document-fingerprint-backfill.rs`
- Inspect/modify if needed: `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`
- Update: `docs/validation/datamax-main-gap-closure.md`
- Reference: `docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md`

**Step 1: Run a reviewed dry run**

On 8 server, use a reviewed dataset or document scope:

```powershell
ssh 8服务器 "cd /srv/aiv3/repo && set -a && . /etc/aiv3/aiv3.env && set +a && ./target/release/document-fingerprint-backfill --limit 20 --dry-run --pretty"
```

Expected:

- candidate, skipped, canonical, and duplicate counts are recorded;
- no content, credentials, or raw document text is printed.

**Step 2: Run limited non-dry-run only after review**

Use the same scoped command without `--dry-run` only for the reviewed target.

Expected:

- fingerprints are recorded;
- duplicate aliasing does not change external document IDs;
- rollback note is documented as "disable enrichment flags and stop worker", not "delete data".

**Step 3: Enqueue and process one enrichment run**

```powershell
ssh 8服务器 "cd /srv/aiv3/repo && set -a && . /etc/aiv3/aiv3.env && set +a && DOCUMENT_ENRICHMENT_WORKER_ONCE=true DOCUMENT_ENRICHMENT_WORKER_MAX_RUNS=1 ./target/release/document-enrichment-worker"
```

Expected:

- one run is claimed or the empty queue is recorded truthfully;
- successful run writes only compact summaries/facts;
- failed run records sanitized error and retry state.

**Step 4: Run aggregate-quality smoke**

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local
```

Expected:

- resume multi-document project experience questions use deterministic supply;
- elderly-care procedure questions cite parsed/enriched procedure content;
- attendance date/work-hour questions remain readable;
- Xinbai aggregate questions use deterministic aggregate before retrieval.

**2026-06-06 controlled production receipt:** completed for one reviewed document on 8 server. The default recent-20 dry run was unsuitable because all 20 were external objects without local files. A narrowed absolute-path document dry run reported `candidate_count=1`, `would_record_count=1`, `skipped_count=0`, `duplicate_count=0`; the matching non-dry-run recorded one canonical fingerprint. A single `fact_index_v2` enrichment run was then enqueued and processed with `DOCUMENT_ENRICHMENT_WORKER_KIND=fact_index_v2`; it succeeded on the first attempt, persisted 56 document facts, and refreshed the dataset entity snapshot to `source_fact_count=210`, `scanned_document_count=6`. No full backfill or long-running worker was enabled.

### P1 Task G: Enable Passive Low-Quality Recovery Safely

**Files:**

- Inspect/modify if needed: `crates/platform-api/src/lib.rs`
- Inspect/modify if needed: `crates/codex-host-agent/src/main.rs`
- Update: `docs/operations/answer-quality-autofix.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Keep the hard gate disabled**

Verify config and behavior:

- no customer-facing hard quality gate is enabled;
- insufficient-evidence answers are not suppressed;
- passive collection can still record weak-answer packages.

**Step 2: Run local regressions**

```powershell
cargo fmt --check -p platform-api -p codex-host-agent
cargo test -p platform-api answer_quality_autofix --lib
cargo test -p platform-api assistant_run_answer_quality_gate --lib
cargo test -p codex-host-agent answer_quality --lib
powershell -ExecutionPolicy Bypass -File .\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary
powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local
```

Expected:

- weak answers and dissatisfaction signals are packaged;
- `blocking_gate_enabled=false`;
- fixed-task allowlist is limited to answer/retrieval optimization;
- no customer answer is blocked by this path.

**Step 3: Production rollout rule**

Live enqueue requires all of these:

- `CODEX_HOST_TASK_ENABLED=true`;
- `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED=true`;
- `CODEX_HOST_TASK_ALLOWLIST` contains `answer_quality_autofix`;
- `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES` contains `answer_quality_autofix`.

If enabled, start with passive collection and manual review only. Do not auto-merge any code change without tests and deployment receipt. If the dedicated autofix flag is absent or false, the system may collect passive cases and record preflight rejection events, but it must not create Codex Host tasks.

### P1 Task H: Expand Operator Observability

**Files:**

- Modify: `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- Modify if needed: `apps/web/app/lib/external-integrations.js`
- Modify if needed: `apps/web/app/lib/external-integrations.test.mjs`
- Modify if needed: `crates/platform-api/src/lib.rs`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Add compact sanitized panels**

The operator page should show:

- model lane health and active concurrency;
- workflow queue backlog by kind;
- static-page/report queued/running/completed/failed counts;
- template reuse/prewarm status;
- document enrichment backlog;
- low-quality passive case count;
- data-ingestion staging/sync state and latest sanitized failure kind.

**Step 2: Keep details lazy-loaded**

Do not load raw logs, raw prompts, raw documents, raw source rows, database URLs, or credentials. Detailed panels load only when selected and remain sanitized.

**Step 3: Verify**

```powershell
npm --prefix apps/web run build
git diff --check
```

If a focused web test exists, run it and record the exact command. If not, add one before changing behavior-heavy UI state.

**Step 4: Commit**

```powershell
git add apps/web crates docs/validation
git commit -m "Expand DataMax operator observability"
git push
```

### Implementation Boundaries For The Next Slices

- Data ingestion confirmation may add internal/operator endpoints, events, UI actions, and smoke scripts, but must not change public third-party message fields.
- Report export support may add artifact metadata and downloadable files, but the customer-visible answer should show one primary report link only.
- Static-page prewarm must never block ordinary Q&A; it should run only when a matching accepted template does not already exist and the low-load guard passes.
- Background enrichment must be asynchronous, idempotent by document fingerprint and enrichment kind, and safe to retry.
- Passive answer-quality recovery must collect and diagnose; it must not suppress normal customer answers.
- Controlled streaming must separate progress/status from final answer release in third-party channels.

## Priority Gates

### P0 Gate A: Production Concurrency

DataMax must prove on 8 server that:

- third-party ordinary chat supports 20+ concurrent conversations;
- main-site ordinary chat supports 20 concurrent active conversations;
- heavy static-page/Image2/HTML jobs are isolated and bounded at 5-way concurrency;
- Cloudflare Codex fallback is bounded at 2-way concurrency;
- ordinary chat latency is not starved by heavy jobs.

### P0 Gate B: Report/Static-Page Operations

DataMax must keep the accepted Xinbai report template reusable, focused, exportable, and observable:

- one customer-visible report link;
- correct `focus`;
- export files accessible;
- template reuse before expensive generation;
- background refresh status visible to operators.

### P1 Gate C: Background Enterprise Memory

Documents and database-derived rows must keep improving after ingestion:

- fingerprint and dedup;
- section/table/entity extraction;
- fact snapshots;
- deterministic aggregates before retrieval;
- no duplicate counting across aliases.

### P1 Gate D: Low-Quality Answer Recovery

DataMax must detect low-quality answers without blocking normal answers:

- collect weak answers and customer dissatisfaction signals;
- classify root cause;
- run fixed-scope `answer_quality_autofix` only for system defects;
- require local regression and smoke before deployment.

### P1 Gate E: Confirmed Data Ingestion

`data_ingestion_analysis` must become a safe staging-to-dataset workflow:

- analysis and staging plan first;
- human/operator confirmation before dataset mutation;
- sync results become normal DataMax dataset evidence;
- no live database rows directly in model context.

---

## Task 1: Create A Main Gap Validation Ledger

**Status:** completed on 2026-06-06.

**Files:**

- Create: `docs/validation/datamax-main-gap-closure.md`
- Modify: `docs/validation/README.md`
- Reference: `docs/validation/external-capability-routing-smoke.md`
- Reference: `docs/validation/external-report-export-smoke.md`

**Step 1: Add the validation ledger**

Create `docs/validation/datamax-main-gap-closure.md` with these sections:

```markdown
# DataMax Main Gap Closure Validation

## Current 8-Server Baseline

- Date:
- Commit:
- Services:
- Model gateway status:
- Worker concurrency config:
- Database pool config:

## Gate Results

| Gate | Status | Receipt |
| --- | --- | --- |
| P0 Gate A: 20-way concurrency | pending | |
| P0 Gate B: report/static-page operations | pending | |
| P1 Gate C: background enterprise memory | pending | |
| P1 Gate D: low-quality answer recovery | pending | |
| P1 Gate E: confirmed data ingestion | pending | |

## Rollout Receipts

Append one dated receipt per deployment.
```

**Step 2: Link it from the validation index**

Modify `docs/validation/README.md` and add the new validation doc.

**Step 3: Record the current deployed baseline**

Run:

```powershell
ssh 8服务器 "cd /srv/aiv3/repo && git rev-parse --short HEAD && systemctl is-active aiv3-platform-api.service && systemctl is-active aiv3-web.service || true && systemctl is-active aiv3-codex-host-agent.service || true"
```

Expected:

- commit is captured;
- `aiv3-platform-api.service` is `active`;
- unavailable optional services are recorded truthfully.

**Step 4: Verify**

Run:

```powershell
git diff --check
```

Expected: no whitespace errors.

**Step 5: Commit**

```powershell
git add docs/validation/datamax-main-gap-closure.md docs/validation/README.md
git commit -m "Add DataMax main gap validation ledger"
```

---

## Task 2: Finish 8-Server 20-Way Concurrency Validation

**Status:** in progress as of 2026-06-06. Read-only 8-server status checks are recorded; private bearer/cookie smoke remains required.

**Files:**

- Modify if needed: `scripts/smoke/external-channel-20way.mjs`
- Modify if needed: `scripts/smoke/main-chat-20way.mjs`
- Modify if needed: `scripts/smoke/static-page-5way.mjs`
- Modify if needed: `scripts/smoke/cloudflare-fallback-2way.mjs`
- Modify: `docs/validation/datamax-main-gap-closure.md`
- Reference: `docs/plans/2026-05-31-v3-20-way-concurrency-upgrade-plan.md`
- Reference: `docs/operations/model-gateway-rollout.md`

**Step 1: Run read-only status checks**

Run on 8 server:

```powershell
ssh 8服务器 "cd /srv/aiv3/repo && curl -fsS http://127.0.0.1:3001/v1/model-gateway/status || true && curl -fsS http://127.0.0.1:3001/v1/workflow-tasks/queue-stats || true"
```

Expected:

- model gateway status is reachable or the exact route mismatch is recorded;
- workflow queue stats are reachable;
- output does not expose secrets.

**Step 2: Run third-party 20-way smoke**

Use private bearer/config only. Do not print bearer values.

```powershell
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20
```

Expected:

- at least 20 distinct `conversation_external_id` values are used;
- success rate is recorded;
- P50/P95 latency is recorded;
- no duplicate assistant runs for the same idempotency key;
- same-conversation overlap returns processing/ordered behavior instead of out-of-order answers.

**Step 3: Run main-site 20-way smoke**

Use the existing private cookie/session config.

```powershell
npm run smoke:main-chat-20way -- --base-url https://v3.elepcloud.com --concurrency 20
```

Expected:

- 20 active conversations complete or return a truthful processing state;
- main-site answers use the assistant chat model lane;
- no worker saturation deadlock.

**Step 4: Run heavy static-page 5-way smoke**

```powershell
npm run smoke:static-page-5way -- --base-url https://v3.elepcloud.com --concurrency 5
```

Expected:

- at most 5 heavy jobs are running;
- ordinary chat remains responsive during the smoke;
- no generated artifact overwrites a stable customer URL without confirmation.

**Step 5: Run Cloudflare fallback 2-way guard smoke**

```powershell
npm run smoke:cloudflare-fallback-2way -- --base-url https://v3.elepcloud.com
```

Expected:

- fallback concurrency cap is 2;
- the smoke does not enqueue expensive jobs unless explicitly configured to do so;
- queue counts match the cap.

**Step 6: Patch scripts only if evidence is incomplete**

If any smoke cannot record latency, run ids, idempotency status, or queue status, patch only the relevant smoke script to add sanitized receipts under `target/`.

**Step 7: Record results**

Update:

- `docs/validation/datamax-main-gap-closure.md`;
- `docs/plans/2026-05-31-v3-20-way-concurrency-upgrade-plan.md`.

**Step 8: Verify**

Run:

```powershell
npm --prefix apps/web run build
cargo check -p platform-api
git diff --check
```

Expected: pass, unless only docs/scripts changed and the build is explicitly deferred with a reason.

**Step 9: Commit**

```powershell
git add scripts/smoke docs/validation docs/plans/2026-05-31-v3-20-way-concurrency-upgrade-plan.md
git commit -m "Validate DataMax 20-way concurrency on 8 server"
```

---

## Task 3: Lock Production Concurrency Configuration

**Status:** partially closed as of 2026-06-06. Template reuse and explicit report/static-page operations are validated. Prewarm task construction exists, and the silent prewarm source can now pass through the later auto-publish path without customer outbound replies. 8-server prewarm env is not enabled yet because a real low-load prewarm consumer/execution smoke still needs closure.

**Files:**

- Modify if needed: `docs/operations/model-gateway-rollout.md`
- Modify if needed: `crates/platform-api/src/lib.rs`
- Modify if needed: `crates/storage/src/lib.rs`
- Modify if needed: `crates/static-page-worker/src/main.rs`
- Modify if needed: `crates/codex-host-agent/src/main.rs`
- Modify: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Compare expected and actual 8-server runtime knobs**

Inspect without printing secrets:

```powershell
ssh 8服务器 "systemctl show aiv3-platform-api.service -p Environment --no-pager | sed -E 's/(KEY|TOKEN|SECRET|PASSWORD|DATABASE_URL)=[^ ]+/\\1=REDACTED/g'"
ssh 8服务器 "systemctl show aiv3-static-page-worker.service -p Environment --no-pager 2>/dev/null | sed -E 's/(KEY|TOKEN|SECRET|PASSWORD|DATABASE_URL)=[^ ]+/\\1=REDACTED/g' || true"
ssh 8服务器 "systemctl show aiv3-codex-host-agent.service -p Environment --no-pager 2>/dev/null | sed -E 's/(KEY|TOKEN|SECRET|PASSWORD|DATABASE_URL)=[^ ]+/\\1=REDACTED/g' || true"
```

Expected runtime targets:

- assistant chat model lane: Right `gpt-5.5` max concurrency 20;
- fallback model lane: MiniMax max concurrency 6;
- main chat worker concurrency: 20 or a documented bounded value that still passes 20-way smoke;
- static-page/Image2/HTML concurrency: 5;
- Cloudflare fallback concurrency: 2;
- DB pool caps high enough for the worker count but not unbounded.

**Step 2: Patch code only if config cannot be enforced operationally**

If code defaults or clamps prevent the target configuration, add targeted tests first, then patch.

Example tests:

```powershell
cargo test -p platform-api model_gateway --lib
cargo test -p static-page-worker concurrency --lib
cargo test -p codex-host-agent concurrency --lib
```

**Step 3: Update operator docs**

Document the expected 8-server profile and rollback knobs in `docs/operations/model-gateway-rollout.md`.

**Step 4: Verify**

Run:

```powershell
cargo fmt --check
cargo check -p platform-api
cargo test -p platform-api model_gateway --lib
git diff --check
```

**Step 5: Commit**

```powershell
git add crates docs/operations/model-gateway-rollout.md docs/validation/datamax-main-gap-closure.md
git commit -m "Lock DataMax production concurrency profile"
```

---

## Task 4: Close Static-Page Template Operations And Prewarm

**Status:** partially closed as of 2026-06-06. Accepted-template reuse and the Xinbai report template contract are validated locally/publicly. Prewarm task construction exists, the silent prewarm source is allowed through auto-publish, and prewarm publish completion/failure is protected from unsolicited customer outbound replies. Production low-load prewarm remains disabled on 8 server until a real prewarm consumer/low-load execution smoke is recorded.

**Files:**

- Modify if needed: `crates/platform-api/src/lib.rs`
- Modify if needed: `crates/static-page-worker/src/lib.rs`
- Modify if needed: `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- Modify if needed: `apps/web/app/lib/external-integrations.js`
- Modify if needed: `docs/static-page-templates/xinbai-functional-modular-template-20260604/template-contract.json`
- Modify: `docs/plans/2026-05-30-static-page-image-pipeline-optimization-plan.md`
- Modify: `docs/validation/external-report-export-smoke.md`
- Modify: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Preserve accepted-template-first behavior**

Add or verify a regression proving:

- same dataset combination returns the accepted template;
- dataset-overlap can reuse the accepted template;
- same `default_prompt` with overlapping source scope prefers the same template;
- explicit redesign request bypasses reuse;
- ordinary Q&A does not trigger report generation.

Run:

```powershell
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api external_channel_capability_routing_fixture --lib
```

**Step 2: Add low-load template prewarm policy**

Implement a background-only policy:

- if a conversation has a document/dataset scope plus `default_prompt`;
- and no accepted template exists for the overlapping data domain and prompt family;
- and system load is below the configured threshold;
- enqueue a silent template prewarm task;
- do not tell the customer unless the customer requested a report/page;
- do not enqueue if any overlapping dataset already has an accepted template.

Use additive internal events such as:

- `assistant_run.static_page_template_prewarm_considered`;
- `assistant_run.static_page_template_prewarm_skipped`;
- `assistant_run.static_page_template_prewarm_queued`.

Do not add required public third-party fields.

**Step 3: Expose prewarm and reuse in operator view**

Update the external integrations observability page to show:

- accepted template id;
- match policy;
- focus;
- background refresh status;
- prewarm skipped reason;
- queue P50/P95 where available.

**Step 4: Validate Xinbai template contract**

Run:

```powershell
npm run validate:xinbai-report-template
npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main
```

Expected:

- template files pass local and public validation;
- JSON and SSE both expose one usable report surface;
- exported files are accessible.

**Step 5: Commit**

```powershell
git add crates apps/web docs/static-page-templates docs/plans/2026-05-30-static-page-image-pipeline-optimization-plan.md docs/validation
git commit -m "Close static page template reuse and prewarm operations"
```

---

## Task 5: Implement Background Document Enrichment Phase 1

**Status:** completed for the post-ingest rollout as of 2026-06-06. Storage schema, third-party parse fingerprint capture, main-site local register fingerprint capture, zip child-document fingerprint capture, a dry-run capable existing-document fingerprint backfill tool, canonical read-through for chunks/evidence/facts, the `document_enrichment_runs` repository foundation, feature-flagged post-ingest enrichment enqueue, document-level enrichment diagnostics, the low-priority enrichment worker execution loop, and 8-server newly parsed document enrichment are implemented. A tiny live upload/new-parse smoke on 8 server proved enqueue/consume for the configured deterministic enrichment kinds. Production non-dry-run full backfill remains disabled.

**Files:**

- Modify: `crates/storage/migrations/mod.rs` or migration registry if present
- Create: `crates/storage/migrations/0013_document_canonical_enrichment.sql`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/ingest-worker/src/lib.rs`
- Create: `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`
- Modify: `docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md`
- Modify: `docs/validation/document-understanding-smoke.md`

**Step 1: Add schema tests first**

Add tests proving the migration includes:

- `content_sha256`;
- `content_size_bytes`;
- `canonical_document_id`;
- `dedup_state`;
- `document_content_fingerprints`;
- `document_enrichment_runs`;
- idempotency index on `(tenant_id, document_id, enrichment_kind, input_fingerprint)`.

Run:

```powershell
cargo test -p storage document_canonical_enrichment --lib
```

Expected: fail before implementation.

**Step 2: Add migration and storage structs**

Implement the schema in `0013_document_canonical_enrichment.sql` and storage mappings in `crates/storage/src/lib.rs`.

**Step 3: Capture fingerprints during local upload and third-party parse**

Compute SHA-256 and byte size while bytes are already in memory or streaming through the download path.

Do not change third-party parse request/response shape.

**Step 4: Keep duplicate document IDs stable**

If a duplicate is detected:

- keep the new document row for external traceability;
- set `canonical_document_id`;
- set `dedup_state=duplicate`;
- do not delete the object file in this rollout;
- ensure dataset membership still authorizes the content.

**Step 5: Verify**

Run:

```powershell
cargo fmt --check
cargo test -p storage document_canonical_enrichment --lib
cargo test -p platform-api external_document_parse --lib
cargo test -p platform-api document_dataset_membership --lib
cargo check -p ingest-worker
```

**Step 6: Commit**

```powershell
git add crates/storage crates/platform-api crates/ingest-worker docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md docs/validation/document-understanding-smoke.md
git commit -m "Add background document enrichment fingerprints"
```

---

## Task 6: Implement Background Fact Enrichment Phase 2

**Status:** completed for the post-ingest rollout as of 2026-06-06. Background enrichment worker now supports `table_structure_v1`, `entity_terms_v1`, `procedure_steps_v1`, `resume_profile_v1`, and `spreadsheet_metrics_v1` in addition to the Phase 1 kinds. Post-ingest enrichment enqueue includes the new kinds when `DOCUMENT_ENRICHMENT_ENABLED=true`, and 8 server now narrows live post-ingest work to `fact_index_v2`, `table_structure_v1`, `procedure_steps_v1`, `resume_profile_v1`, and `spreadsheet_metrics_v1`. `fact_index` emits `procedure_step` and `time_threshold` facts for care-operation text and includes those fact types in dataset entity snapshots. Fixture coverage has been added for elderly-care procedure facts and enrichment-specific resume/table/attendance checks. Full local document-quality smoke passed on 2026-06-06. A tiny live 8-server upload/new-parse smoke confirmed all five configured kinds succeeded.

**Files:**

- Modify: `crates/platform-api/src/fact_index.rs`
- Modify: `crates/retrieval-worker/src/main.rs`
- Modify: `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`
- Modify: `crates/ingest-worker/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Create or modify fixtures under: `fixtures/document-quality/`
- Modify: `scripts/run-document-quality-smoke.ps1`
- Modify: `docs/validation/document-understanding-smoke.md`

**Step 1: Add fixture coverage for known weak domains**

Add focused cases for:

- elderly-care manual:
  - 翻身频率;
  - 发药核对;
  - 护理交接班;
  - 跌倒/离世/应急处置;
- resume corpus:
  - project experience across many resumes;
  - company count;
  - multi-dimensional ranking table;
- attendance spreadsheet:
  - absence;
  - work-hour length;
  - date formatting.

**Step 2: Add enrichment kinds**

Support queued enrichment kinds:

- `section_outline`;
- `table_structure`;
- `entity_terms`;
- `procedure_steps`;
- `resume_profile`;
- `spreadsheet_metrics`.

Each enrichment run must store:

- status;
- parse version;
- input fingerprint;
- short output summary;
- safe error message.

Local progress:

- Implemented aliases for the plan names and versioned worker kinds.
- Added deterministic summaries for table structures, entity terms, procedure steps, resume profiles, and spreadsheet/attendance metrics.
- Kept the worker standalone and low-priority; no model calls or VLM reparse are introduced here.

**Step 3: Improve fact ranking before the cap**

Extend `crates/platform-api/src/fact_index.rs` so it:

- filters TOC dot-leader noise;
- keeps facts from later sections;
- prefers procedure headings and concrete step lists;
- extracts company/project/role/time facts from resumes;
- extracts date/hour/attendance facts from spreadsheets;
- keeps source locators.

Local progress:

- Added `procedure_step` and `time_threshold` fact types to the dataset entity snapshot type list.
- Added conservative procedure sentence and time-threshold extraction for care manuals.
- Boosted procedure/time-threshold fact rank and confidence so late operational manual sections survive the fact cap.

**Step 4: Rebuild dataset fact snapshots after enrichment**

Ensure retrieval-worker or ingest-worker schedules snapshot refresh after enrichment completes.

Local progress:

- Existing `fact_index_v2` enrichment rebuilds document facts and refreshes the dataset entity rows snapshot.
- Duplicate alias enrichment runs skip fact writes and report canonical read-through instead of double-counting.

**Step 5: Verify**

Run:

```powershell
cargo fmt --check
cargo test -p platform-api fact_index --lib
cargo test -p platform-api document_understanding --lib
cargo test -p retrieval-worker
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1
```

**Step 6: Commit**

```powershell
git add crates fixtures scripts/run-document-quality-smoke.ps1 docs/validation/document-understanding-smoke.md
git commit -m "Add background fact enrichment coverage"
```

---

## Task 7: Put Deterministic Facts Before Retrieval For Aggregates

**Status:** completed locally on 2026-06-06; 8-server private aggregate smoke remains pending because private bearer/cookie config is not present in this local shell.

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/fact_index.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify if needed: `crates/platform-api/src/react_agent_catalog.rs`
- Modify: `docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md`
- Modify: `docs/validation/document-understanding-smoke.md`

**Step 1: Add tests for aggregate-first supply**

Tests must prove:

- global/cross-document count questions use `dataset_fact_snapshot` before top-k retrieval;
- selected document scopes use `document_facts_scoped_aggregate`;
- duplicate aliases are not counted twice;
- retrieval evidence remains available as secondary support;
- top-k retrieval alone cannot answer multi-document aggregate questions.

Run:

```powershell
cargo test -p platform-api dataset_fact_snapshot --lib
cargo test -p platform-api scoped_fact --lib
```

**Step 2: Add an aggregate intent detector**

Detect questions involving:

- count/statistics;
- ranking/sorting;
- company/project/person/location/category;
- absence/work hours/date;
- store/brand/risk/opportunity metrics.

The detector must be internal and must not change public contracts.

**Step 3: Update model-facing supply brief**

Make the brief explicit:

- use deterministic aggregate rows as authoritative for totals;
- do not infer totals from retrieval chunks;
- cite scope and row count;
- use retrieval evidence only for explanation/details.

Local progress:

- Added an internal deterministic aggregate intent detector covering:
  - document/entity count, list, rank, and table questions;
  - resume company/project/skill/position/location/education/certificate/experience ranking;
  - attendance absence, work-hour, and date row questions;
  - store/brand/category/risk/opportunity/take-high/low-active business metrics.
- Kept business-metric aggregate detection separate from generic document entity scans so database-backed operating questions can use database aggregates without forcing a full document scan unless the user explicitly points at documents, files, attachments, tables, or parsed materials.
- Tightened model-facing guidance for `dataset_fact_snapshot`, `document_facts_scoped_aggregate`, `database_aggregate`, `dataset_entity_scan`, and `spreadsheet_row_analysis`:
  - cite deterministic scope and row counts;
  - use `row_count_by_type`, `scanned_document_count`, `source_document_count`, `source_fact_count`, `scan_limit`, and `result_row_count` where applicable;
  - treat retrieval chunks as examples/source wording only, not proof of full-dataset totals.
- Updated the ReAct planning catalog so models see deterministic aggregate supply actions before retrieval top-k for aggregate/statistical questions.

**Step 4: Verify with private smoke**

Run local and 8-server cases:

- 简历公司名统计;
- 14 份简历项目经验排序;
- 考勤缺勤/工时长短;
- 新百风险/取高/低活跃统计;
- 养老手册操作规范问答.

Local verification on 2026-06-06:

```powershell
cargo fmt --check -p platform-api
cargo test -p platform-api dataset_fact_snapshot --lib
cargo test -p platform-api scoped_fact --lib
cargo test -p platform-api database_aggregate_heuristics --lib
cargo test -p platform-api assistant_run_general_entity_scan_prompts_request_dataset_scan --lib
cargo test -p platform-api assistant_run_deterministic_aggregate_intent_covers_customer_smoke_domains --lib
cargo test -p platform-api planning_catalog_prefers_aggregate_fact_supply_without_fact_rows --lib
cargo test -p platform-api assistant_run_answer_quality_judge_runs_for_structured_short_answer --lib
cargo test -p platform-api assistant_run_answer_quality_judge_skips_satisfied_spreadsheet_table --lib
cargo test -p platform-api assistant_run_answer_quality_gate_retries_deferred_retrieval_language --lib
cargo check -p platform-api
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local
```

Full local smoke receipt: `target/document-quality-smoke/document-quality-smoke-20260606T010841Z-27544.json` and `target/document-quality-smoke/document-quality-smoke-20260606T010841Z-27544.md`.

8-server private smoke remains pending until the private bearer/cookie configuration is available.

**Step 5: Commit**

```powershell
git add crates docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md docs/validation/document-understanding-smoke.md
git commit -m "Prefer deterministic facts for aggregate answers"
```

---

## Task 8: Add Low-Quality Answer Monitoring And Fixed-Scope Autofix

**Status:** completed locally on 2026-06-06; production enqueue remains configuration-gated by `CODEX_HOST_TASK_ENABLED`, `CODEX_HOST_TASK_ALLOWLIST`, `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES`, and the dedicated `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED` opt-in flag.

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `docs/operations/answer-quality-autofix.md`
- Modify: `scripts/run-v3-quality-gate-smoke.ps1`
- Modify: `scripts/run-cloudflare-codex-fixed-task-smoke.ps1`
- Create or modify fixtures under: `fixtures/document-quality/`
- Modify: `docs/validation/document-understanding-smoke.md`
- Modify: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Keep hard gate disabled**

Add a regression proving normal answers are not blocked merely because the draft contains cautious language.

Run:

```powershell
cargo test -p platform-api answer_quality --lib
```

**Step 2: Add passive low-quality signals**

Collect internal signals from completed runs:

- answer says no evidence while evidence exists;
- answer exposes internal tool call text;
- answer lacks required report link after report trigger;
- answer contradicts deterministic fact rows;
- customer expresses dissatisfaction;
- repeated fallback/timeout for the same question family.

Store only safe summaries and references. Do not store full customer documents.

Local progress:

- Passive case collection remains post-answer and records `blocking_gate_enabled=false`.
- Added passive signals for:
  - deterministic aggregate supply ignored by an insufficient-evidence answer;
  - report/static-page request completed without a usable generated-artifact link;
  - repeated fallback/timeout/provider-failure events in the same run.
- Existing signals remain covered for user complaints, strong complaints, internal marker/tool-call leaks, weak insufficient answers, retry exhaustion, controlled fallback, and parse-quality degradation without VLM upgrade.

**Step 3: Route only system-defect cases to fixed-task autofix**

Use existing `answer_quality_autofix` with strict write scope:

- `crates/platform-api/src/lib.rs`;
- `fixtures/document-quality/**`;
- `scripts/run-document-quality-smoke.ps1`;
- `scripts/run-v3-quality-gate-smoke.ps1`.

Reject:

- missing customer data;
- ambiguous user request;
- unsupported public contract change;
- required credential access.

Local progress:

- Fixed-task allowlist remains restricted to answer-quality files, document-quality fixtures, smoke scripts, and validation docs.
- `answer_quality_autofix` output validation still rejects out-of-scope files, missing tests, missing rollback notes, invalid failure types, and high-risk patches without human review.
- Operation docs now match the actual legacy smoke script path `scripts/run-v3-quality-gate-smoke.ps1`.

**Step 4: Require regression before patch is accepted**

Autofix output must include:

- changed files;
- tests added;
- exact test commands;
- risk level;
- rollback notes.

**Step 5: Verify**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary
powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1
cargo test -p codex-host-agent answer_quality --lib
```

Local verification on 2026-06-06:

```powershell
cargo fmt --check -p platform-api -p codex-host-agent
cargo test -p platform-api answer_quality_autofix --lib
cargo test -p codex-host-agent answer_quality --lib
cargo test -p platform-api assistant_run_answer_quality_gate --lib
powershell -ExecutionPolicy Bypass -File .\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary
powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local
cargo check -p platform-api -p codex-host-agent
```

Smoke receipts:

- Fixed-task smoke JSON: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T011735Z.json`.
- Fixed-task smoke Markdown: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T011735Z.md`.
- Quality-gate smoke JSON: `target/document-quality-smoke/document-quality-smoke-20260606T011549Z-23164.json`.
- Quality-gate smoke Markdown: `target/document-quality-smoke/document-quality-smoke-20260606T011549Z-23164.md`.

**Step 6: Commit**

```powershell
git add crates fixtures scripts docs/operations/answer-quality-autofix.md docs/validation
git commit -m "Add passive answer quality autofix loop"
```

---

## Task 9: Close Confirmed Data Ingestion To Dataset Sync

**Status:** locally upgraded and partially validated on 8 server as of 2026-06-06. Local confirmation/sync contract passed. 8-server live source readiness passed, and an external data-ingestion analysis run completed with a staging plan. Local code now supports a configured model-gateway operator confirming and syncing external-channel staging plans; live 8-server confirm/sync remains pending until this slice is deployed and exercised with an authenticated operator session.

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `crates/ingest-worker/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify if needed: `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- Modify: `scripts/run-data-ingestion-staging-sync-smoke.sh`
- Modify: `scripts/run-data-ingestion-staging-live-smoke.sh`
- Modify: `docs/validation/data-ingestion-analysis-smoke.md`
- Modify: `docs/validation/data-ingestion-staging-sync-smoke.md`
- Modify: `docs/plans/2026-05-21-database-source-integration-plan.md`

**Step 1: Preserve read-only analysis behavior**

Run:

```powershell
bash scripts/run-data-ingestion-staging-sync-smoke.sh
```

Expected:

- `data_ingestion_analysis` produces a staging plan;
- no production dataset write happens without confirmation.

**Step 2: Add explicit confirmation command path**

Implement an internal confirmation action that takes an existing `v3_data_ingestion_staging_plan` and starts dataset creation/update.

Required behavior:

- validate tenant/operator authorization;
- validate source scope;
- validate staging plan schema;
- keep raw credentials server-side;
- write a normal DataMax dataset/document/sync trail;
- append AssistantRun events:
  - `assistant_run.data_ingestion_staging_dataset_ready`;
  - `assistant_run.data_ingestion_staging_sync_started`;
  - `assistant_run.data_ingestion_staging_sync_running`;
  - `assistant_run.data_ingestion_staging_sync_completed`;
  - `assistant_run.data_ingestion_staging_sync_failed`.

**Step 3: Add operator UI affordance**

In external integrations observability, show:

- staging plan summary;
- confirmation-required status;
- safe row/table summary;
- sync progress after confirmation.

Do not expose credentials or raw database rows.

Local progress:

- Internal operator routes are present:
  - `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/confirm`;
  - `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/sync`.
- Confirmation loads only a `v3_data_ingestion_staging_plan` already attached to the AssistantRun output artifacts.
- Confirmation creates or reuses a private DataMax staging dataset and records `assistant_run.data_ingestion_staging_plan_confirmed`.
- Sync refuses unconfirmed plans, validates the selected database source is included in the plan, starts `ExternalSourceSync` into the confirmed dataset, records `assistant_run.data_ingestion_staging_sync_started`, and deduplicates repeated sync clicks by default.
- Workflow progress records sanitized `assistant_run.data_ingestion_staging_sync_updated` events; third-party reply recovery maps them to running/completed/failed states.
- Same `conversation_external_id` follow-up turns can restore a completed staging dataset as visible scope after sync.
- Third-party public request fields are unchanged; docs describe only additive statuses/cards.

**Step 4: Verify**

Run:

```powershell
cargo fmt --check
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p codex-host-agent data_ingestion --lib
bash scripts/run-data-ingestion-staging-sync-smoke.sh
bash scripts/run-data-ingestion-staging-live-smoke.sh
npm --prefix apps/web run build
```

Local verification on 2026-06-06:

```powershell
bash scripts/run-data-ingestion-staging-sync-smoke.sh
```

Passed checks:

- `cargo test -p platform-api data_ingestion --lib` passed, 13 tests.
- `cargo test -p platform-api external_source_sync --lib` passed, 6 tests.
- `cargo test -p external-source-worker` passed, 11 tests.
- `cargo test -p ingest-worker` passed.
- `cargo test -p retrieval-worker` passed.
- `DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true bash scripts/run-data-ingestion-staging-live-smoke.sh` passed through the wrapper.
- `npm run check:pure-third-party-guide-html` passed.

Smoke receipts:

- JSON: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T012507Z.json`.
- Markdown summary: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T012507Z.md`.
- Live readiness self-test JSON: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T012938Z.json`.
- Live readiness self-test Markdown: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T012938Z.md`.

Remaining:

- deploy the operator confirmation path to 8 server without changing third-party public fields;
- manually confirm one reviewed staging plan and start sync through the internal operator route using an authenticated operator session;
- record that the synced dataset contains source-derived documents, chunks, retrieval evidence, and no raw credentials/table dumps.

Local operator-confirmation upgrade on 2026-06-06:

- Added a narrow owner-or-operator loader for data-ingestion staging plan confirm/sync routes.
- Anonymous and ordinary non-owner users still receive `assistant_run_not_found` for external-channel runs.
- Configured model-gateway operators can confirm external-channel staging plans and can access the plan-created private staging dataset for sync.
- The third-party public event/reply API, auth, request fields, and response fields were not changed.
- `bash scripts/run-data-ingestion-staging-sync-smoke.sh` passed and produced `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T025141Z.json`.

8-server verification on 2026-06-06:

- `bash scripts/run-data-ingestion-staging-live-smoke.sh` passed against stored source `hy-sql-traffic-area`.
- External data-ingestion analysis smoke against `https://v3.elepcloud.com` accepted a selected `business_datasource_ids` scope and reached `data_ingestion_analysis_completed`.
- The returned card contained `staging_plan.type=v3_data_ingestion_staging_plan`, `human_review_required=true`, `staging_spec_available=true`, `source_count=1`, `table_count=1`, and `raw_credentials_present=false`.
- Attempting the confirm route without an operator-authenticated session returned HTTP 404 with `assistant_run_not_found`, so this is the next concrete implementation gap rather than a public third-party contract problem.

**Step 5: Commit**

```powershell
git add crates apps/web scripts docs/validation docs/plans/2026-05-21-database-source-integration-plan.md
git commit -m "Close confirmed data ingestion sync flow"
```

---

## Task 10: Roll Out Controlled Streaming Safely

**Status:** partially validated on 8 server as of 2026-06-06. Main-site `continue/stream` uses the same live-delta worker/channel path as new AssistantRun creation when `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true`. Third-party stream behavior and replay safety passed a private 8-server/public-endpoint smoke with active bearer. Main-site browser/SSE smoke remains pending.

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/lib/assistant-run-progress.js`
- Modify: `scripts/smoke/external-channel-streaming-10way.mjs`
- Modify: `docs/plans/2026-06-01-v3-streaming-session-upgrade-plan.md`
- Modify: `docs/integrations/third-party-integration-api.zh-CN.md`
- Modify: `docs/integrations/third-party-integration-api.zh-CN.html`
- Modify: `apps/web/public/external-integrations/third-party-integration-api.zh-CN.md`
- Modify: `apps/web/public/external-integrations/third-party-integration-api.zh-CN.html`

**Step 1: Keep third-party answer safety policy**

Third-party stream should:

- stream progress/status;
- stream artifact links when available;
- release final answer after quality checks;
- avoid leaking rejected model deltas.

**Step 2: Enable main-site live answer stream behind a flag**

Use:

- `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED`;
- existing external flag `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED` only after smoke passes.

Local progress:

- `/v1/assistant-runs/stream` already emitted `assistant_run.delta` behind `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED`.
- `/v1/assistant-runs/{run_id}/continue/stream` now also uses a live worker/channel when that flag is enabled.
- Continue streaming reuses `AssistantRunLiveDeltaSink` and `complete_assistant_run_provider_live_streaming`.
- Continue completion can skip the final full-text delta if live deltas already reached the browser, avoiding duplicate text in the main-site chat UI.
- Non-streaming JSON continue and background model-completion recovery still pass `None` for the live sink.

**Step 3: Add reconnect and final-shape regressions**

Tests must prove:

- `/events`;
- `/events/stream`;
- `/assistant-runs/{id}/reply`;

produce consistent sanitized terminal status.

**Step 4: Verify**

Run:

```powershell
cargo fmt --check
cargo test -p platform-api external_channel_streaming --lib
cargo test -p platform-api assistant_run_streaming --lib
npm run smoke:external-channel-streaming-10way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main
npm --prefix apps/web run build
npm run build:pure-third-party-guide-html
npm run check:pure-third-party-guide-html
```

Local verification on 2026-06-06:

```powershell
cargo fmt --check -p platform-api
cargo test -p platform-api assistant_run_sse --lib
cargo test -p platform-api assistant_run_continue_sse --lib
cargo test -p platform-api assistant_run_live --lib
cargo test -p platform-api assistant_run_continue --lib
cargo test -p platform-api external_channel_stream_resume --lib
cargo test -p platform-api external_channel_public_stream --lib
cargo test -p platform-api generic_chat_page_event_stream_can_emit_live_answer_delta_without_final_duplication --lib
cargo check -p platform-api
```

Notes:

- `cargo test -p platform-api assistant_run_streaming --lib` currently has no matching tests; the actual main-site filters above are the authoritative local coverage for this slice.
- `cargo test -p platform-api external_channel_streaming --lib` currently has no matching tests; third-party stream coverage is under `external_channel_public_stream`, `external_channel_stream_resume`, and the named generic-chat live delta test.

Remaining:

- run browser/local smoke for main-site new and continued AssistantRun streaming with `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true`;
- keep recording whether `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED=true` should release final answer deltas or remain progress/artifact-first for third-party clients.

8-server external streaming verification on 2026-06-06:

- Command shape: `node scripts/smoke/external-channel-streaming-10way.mjs --base-url https://v3.elepcloud.com --connection-id generic-chat-main --timeout-ms 180000`.
- Receipt: `/srv/aiv3/repo/target/external-channel-streaming-10way-smoke/20260606020619.json`.
- Result: 15/15 tasks OK, including 10 ordinary, 3 static-page, and 2 reconnect cases.
- Duplicate final messages: 0.
- Artifact links: 3.
- Latency: P50 3235 ms, P95 6015 ms, max 6015 ms.

**Step 5: Commit**

```powershell
git add crates apps/web scripts docs/plans/2026-06-01-v3-streaming-session-upgrade-plan.md docs/integrations apps/web/public/external-integrations
git commit -m "Roll out controlled DataMax streaming"
```

---

## Task 11: Expand Operator Observability

**Status:** deployed to 8 server on 2026-06-06; authenticated model-gateway operator smoke remains pending.

**Files:**

- Modify: `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- Modify: `apps/web/app/lib/external-integrations.js`
- Modify: `apps/web/app/lib/external-integrations.test.mjs`
- Modify if needed: `apps/web/app/api/v3/external/codex-executor-tasks/queue-stats/route.js`
- Modify if needed: `crates/platform-api/src/lib.rs`
- Modify: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Add a concise operations summary**

The external integrations page should show:

- ordinary chat health;
- model gateway active lane;
- workflow queue backlog;
- heavy job concurrency;
- template reuse/prewarm status;
- low-quality answer queue count;
- data-ingestion staging/sync status;
- document enrichment backlog.

**Step 2: Keep panels lazy-loaded**

Do not load expensive task/runtime details until the operator expands the panel or selects a task.

**Step 3: Add tests**

Run:

```powershell
npm --prefix apps/web test -- external-integrations
```

If the repo does not expose that exact test command, use the existing package test command and record the actual one in validation.

**Step 4: Build**

Run:

```powershell
npm --prefix apps/web run build
```

Local verification completed:

- `node --test app/lib/external-integrations.test.mjs` from `apps/web` passed, 27 tests.
- `npm --prefix apps/web run build` passed.
- `GET http://127.0.0.1:3100/external-integrations` returned HTTP `200` during local dev-server smoke.
- The new summary uses existing protected/light endpoints and keeps conversation/runtime details lazy-loaded.

8-server verification completed:

- 8 server pulled `2193e0cd5248`, built `apps/web`, and restarted `aiv3-web.service`.
- `GET http://127.0.0.1:3100/external-integrations` returned `200` and included DataMax/运营总览 SSR text.
- `GET https://v3.elepcloud.com/external-integrations` returned `200` and included DataMax/运营总览 SSR text.
- Web queue-stats proxy returned `200` with the configured observability cookie.
- `GET http://127.0.0.1:3000/v1/model-gateway/status` returned `401 auth_session_required` without a main-system operator session, which confirms the protected boundary but does not replace an authenticated operator smoke.

**Step 5: Commit**

```powershell
git add apps/web docs/validation/datamax-main-gap-closure.md
git commit -m "Expand DataMax operator observability"
```

---

## Task 12: Final 8-Server Release Gate

**Status:** pending

**Files:**

- Modify: `docs/validation/datamax-main-gap-closure.md`
- Modify relevant validation docs touched by previous tasks

**Step 1: Local full gate**

Run:

```powershell
cargo fmt --check
cargo check -p platform-api
cargo test -p platform-api external_channel_model_tool_request --lib
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api dataset_fact_snapshot --lib
cargo test -p platform-api scoped_fact --lib
cargo test -p platform-api answer_quality --lib
cargo test -p codex-host-agent
npm --prefix apps/web run build
npm run build:pure-third-party-guide-html
npm run check:pure-third-party-guide-html
git diff --check
```

**Step 2: Push**

```powershell
git status --short --branch
git push
```

Expected:

- no unrelated dirty work is included;
- branch is pushed to GitHub.

**Step 3: Deploy to 8 server**

Use the established build pattern:

```powershell
ssh 8服务器 "cd /srv/aiv3/repo && git pull --ff-only && CC=clang CXX=clang++ cargo build --release -p platform-api && sudo systemctl restart aiv3-platform-api.service && sleep 3 && systemctl is-active aiv3-platform-api.service && git rev-parse --short HEAD"
```

Restart additional worker services only if the task changed their binaries or runtime env:

- `aiv3-static-page-worker.service`;
- `aiv3-codex-host-agent.service`;
- `aiv3-ingest-worker.service`;
- `aiv3-retrieval-worker.service`;
- `aiv3-web.service`.

**Step 4: 8-server private smoke**

Run and record:

```powershell
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20
npm run smoke:main-chat-20way -- --base-url https://v3.elepcloud.com --concurrency 20
npm run smoke:static-page-5way -- --base-url https://v3.elepcloud.com --concurrency 5
npm run smoke:cloudflare-fallback-2way -- --base-url https://v3.elepcloud.com
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-capability-routing-smoke.ps1 -BaseUrl https://v3.elepcloud.com
bash scripts/run-data-ingestion-staging-sync-smoke.sh
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1
```

Required scenario coverage:

- third-party ordinary Q&A still works;
- `取高`, `经营状况`, `风险识别`, `销售缺口/助推` trigger report;
- report link appears once and is clickable;
- report URL carries correct `focus`;
- `table-data.csv`, `report.ppt`, and `report.md` are accessible;
- `取高是什么意思？` and `风险识别系统有哪些项目经历？` do not trigger report;
- resume project-experience aggregate uses deterministic supply;
- elderly-care manual procedure answers cite the relevant parsed content;
- attendance date/work-hour output remains readable;
- data-ingestion analysis reaches a terminal or truthful confirmation-required state;
- no raw credentials or full customer document dumps appear in events.

**Step 5: Record and commit validation**

Update `docs/validation/datamax-main-gap-closure.md` with:

- deployed commit;
- service statuses;
- smoke commands;
- pass/fail table;
- failed cases and root-cause notes;
- rollback path.

Commit:

```powershell
git add docs/validation
git commit -m "Record DataMax main gap closure rollout"
git push
```

---

## Execution Order

Use `Active Execution Sheet - 2026-06-06` as the current execution sheet. Older Task A-H bodies remain implementation history and reference only.

1. Task X1: implement real low-load static-page template prewarm and prove it does not create orphan tasks or unsolicited customer replies.
2. Task X2: rerun explicit report/static-page request smoke and fix only regressions.
3. Final P0 release gate: local tests, deploy changed services, 8-server smoke, validation receipt, commit, push.
4. Task X3: decide passive low-quality recovery rollout. Keep hard answer gate disabled.
5. Task X4: run authenticated model-gateway operator smoke without adding an auth bypass.
6. Task X5: review data-source identity mappings for row-level report completeness.
7. P2: consider limited historical enrichment backfill and template-library cleanup only after P0/P1 are stable.

## Definition Of Done

- 8 server has recorded receipts for the current P0 release gate on the final deployed commit.
- Low-load prewarm either remains disabled with a documented reason or is enabled only after an end-to-end no-customer-reply smoke.
- Prewarm uses real consumed work and cannot leave permanent orphan queue items.
- Third-party ordinary chat, main-site chat, and main-site streaming remain stable after the prewarm slice.
- Explicit Xinbai report requests return one focused link and expose `table-data.csv`, `report.ppt`, and `report.md`.
- Report triggers stay domain-scoped; ordinary questions such as `取高是什么意思？` do not accidentally generate reports.
- Heavy jobs are bounded and do not starve 20-way chat.
- Newly parsed documents continue to enrich without blocking upload or chat.
- Low-quality recovery remains passive, audited, fixed-scope, and does not suppress normal answers.
- Data-ingestion counters truthfully separate source rows, unique documents, chunks, evidence, and collapsed rows.
- Operator observability can diagnose queue, model, report, enrichment, data-ingestion, and low-quality recovery health without raw logs or secrets.
- No public third-party URL, auth method, required request field, or existing response field was changed.
