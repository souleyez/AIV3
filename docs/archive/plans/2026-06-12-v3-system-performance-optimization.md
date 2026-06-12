# V3 System Performance Optimization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve end-to-end V3/DataMax response speed, queue throughput, and perceived latency without reducing answer quality, retrieval recall, parsing coverage, or static-page visual quality.

**Architecture:** Optimize by measuring first, then removing repeated work, parallelizing independent work, using indexed retrieval and structured database query plans, and making long-running work asynchronous with visible progress. Quality-sensitive work such as evidence retrieval, VLM fallback, GPT-Image-2 visual design, and GPT-5.5/DataMax final reasoning remains intact.

**Tech Stack:** Rust `platform-api`/workers/storage, PostgreSQL 17, Next.js Web UI, NATS/workflow task queues, Node smoke scripts, static-page worker, retrieval worker, assistant-run worker, GPT-5.5/DataMax model gateway.

---

## 0. Current Baseline and Rules

### Current Evidence

- Local V3 worktree has unrelated third-party HTML doc changes; do not touch them in this performance work.
- 8 server current code is `376262f`, and `aiv3-platform-api`, `aiv3-web`, `aiv3-assistant-run-worker`, `aiv3-ingest-worker`, `aiv3-retrieval-worker`, `aiv3-static-page-worker`, and `aiv3-media-worker` are active.
- 8 server CPU is not saturated during the last read-only check.
- Main table sizes are modest: `assistant_runs` about 57 MB, `retrieval_evidences` about 15 MB, `document_chunks` about 15 MB.
- Existing bottlenecks are more likely serial orchestration, model wait time, legacy retrieval scanning, repeated context work, static-page executor waits, and insufficient progress feedback.

### Non-Negotiable Quality Rules

- Do not reduce retrieval evidence quality to improve speed.
- Do not reduce VLM/OCR coverage by hard page caps as a performance strategy.
- Do not switch to weaker models for customer-facing default answers.
- Do not shorten timeouts to create false failures.
- Do not skip GPT-Image-2 visual contract for high-quality static pages.
- Do not fail because optional data is thin; expand supply, use available data, and surface data-gap notes.
- Do not expose internal JSON, provider payloads, or giant trace content to users.

### Success Metrics

Record p50/p90/p95 for each major path:

- Ordinary chat: accepted response, first visible progress, first model token, final answer.
- Retrieval: dataset scope resolution, lexical/vector/fact candidate time, evidence pack assembly time.
- Third-party `/events`: sync accepted response, SSE first event, final result, async callback/push.
- Static page: draft creation, Image2 preview, render/publish, Codex executor completion, public URL ready.
- Parsing: upload accepted, quick parse available, full parse complete, retrieval index complete.

Quality gates must not regress:

- Retrieval Recall@20 and MRR@20 on existing fixture/live subset.
- Permission leak count stays `0`.
- NewBai/customer answer evaluator stays passing.
- Static-page artifact validator stays passing.
- Third-party reply does not emit repeated links or giant internal JSON.

---

## Phase 1: Performance Observability Baseline

### Task 1.1: Add AssistantRun Performance Stage Events

**Files:**
- Modify: `crates/storage/migrations/0015_assistant_run_performance_events.sql`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Implementation Steps:**

1. Create migration table `assistant_run_performance_events`:
   - `id uuid primary key`
   - `tenant_id uuid not null`
   - `assistant_run_id uuid not null`
   - `stage text not null`
   - `started_at timestamptz not null`
   - `finished_at timestamptz`
   - `duration_ms bigint`
   - `status text not null default 'ok'`
   - `metadata jsonb not null default '{}'::jsonb`
   - indexes on `(tenant_id, assistant_run_id, started_at)` and `(stage, started_at)`
2. Add storage methods:
   - `record_assistant_run_perf_stage_started`
   - `record_assistant_run_perf_stage_finished`
   - `list_assistant_run_perf_summary`
3. Add low-risk stage recording around existing assistant-run flow:
   - `request_received`
   - `scope_resolution`
   - `retrieval_supply`
   - `database_supply`
   - `prompt_build`
   - `model_call`
   - `reply_finalize`
   - `artifact_dispatch`
4. Metadata must be counts only: evidence count, dataset count, document count, model profile id, backend name, queue name. Do not store raw prompt, raw evidence, raw answer, token, payload, or document body.

**Validation Commands:**

```bash
cargo fmt --check
cargo test -p platform-api assistant_run_performance --lib
cargo test -p storage assistant_run_performance --lib
```

**Acceptance:**

- Stage events are written for a local assistant run.
- Missing performance write must not fail the user request.
- Metadata is redacted and contains no raw customer content.

### Task 1.2: Add Workflow Queue Latency Summary

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/external-integrations.js`
- Modify: `apps/web/app/admin/external-integrations/page.js`
- Test: `apps/web/app/lib/external-integrations.test.mjs`

**Implementation Steps:**

1. Extend existing workflow task stats with:
   - pending count
   - claimed count
   - oldest pending age
   - p50/p95 queue wait
   - p50/p95 execution duration
   - retry count
   - failed count by queue and task key
2. Keep the endpoint protected by existing observability/admin key.
3. In the admin/external observability page, load these stats only when the queue panel is selected.
4. Do not enable continuous polling by default. Use manual refresh and optional short polling only while the panel is visible.

**Validation Commands:**

```bash
cargo test -p platform-api workflow_task_queue_stats --lib
node --test apps/web/app/lib/external-integrations.test.mjs
```

**Acceptance:**

- Queue stats show static page, codex host, ingest, retrieval, assistant run, and external action latency.
- Protected endpoint still returns `401` without auth.
- UI does not load queue stats until selected.

### Task 1.3: Add Read-Only Performance Smoke Script

**Files:**
- Create: `scripts/smoke/system-performance-baseline.mjs`
- Modify: `package.json`
- Create: `docs/validation/system-performance-baseline.md`

**Implementation Steps:**

1. Script inputs:
   - `--base-url`
   - `--observability-key`
   - `--output-dir`
   - `--no-auth` for local limited mode
2. Collect:
   - `/healthz`, `/readyz`
   - queue stats when authorized
   - optional public static page URL timing
   - optional third-party docs page timing
3. Output JSON and Markdown reports.
4. Redact keys, cookies, bearer values, database URLs, internal object paths, and raw payloads.

**Validation Commands:**

```bash
node --check scripts/smoke/system-performance-baseline.mjs
npm run smoke:system-performance-baseline -- --base-url http://127.0.0.1:3000 --no-auth
```

**Acceptance:**

- Baseline report records timings without mutating production state.
- Report can run against 8 server with observability key when approved.

---

## Phase 2: Chat and Retrieval Latency Without Quality Loss

### Task 2.1: Parallelize Independent Context Preparation

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Implementation Steps:**

1. Identify assistant-run context branches that do not depend on each other:
   - conversation history summary
   - selected dataset/document scope restore
   - retrieval evidence search
   - database/fact snapshot supply
   - requested skills/tool capability summary
2. Use bounded `tokio::try_join!` or `JoinSet` for independent branches.
3. Apply a per-branch soft deadline only for optional background context. Required retrieval/database branches must not be silently dropped.
4. Preserve deterministic merge order when building final prompt supply.

**Validation Commands:**

```bash
cargo test -p platform-api assistant_run_context_parallelism --lib
cargo test -p platform-api external_scoped_document_chat --lib
```

**Acceptance:**

- Same evidence count and allowed source list as before.
- Lower measured `retrieval_supply + prompt_build` duration in performance events.
- Permission scopes remain enforced before ranking and supply merge.

### Task 2.2: Session-Level Scope and Evidence Cache

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/migrations/0016_assistant_context_cache.sql`
- Test: `crates/platform-api/src/lib.rs`

**Implementation Steps:**

1. Add cache table for scoped, short-lived context:
   - tenant id
   - conversation external id or assistant thread id
   - dataset/document scope hash
   - user id hash
   - retrieval query normalized hash
   - cached summary JSON
   - expires at
2. Cache only derived summaries and ids, not raw full documents.
3. Reuse cache when:
   - same tenant
   - same conversation
   - same document/dataset scope
   - same or compatible query intent
   - no newer document parse/index timestamp invalidates it
4. Invalidate on new uploaded document, dataset membership change, or database sync completion.

**Validation Commands:**

```bash
cargo test -p platform-api assistant_context_cache_respects_scope --lib
cargo test -p platform-api assistant_context_cache_invalidates_on_document_change --lib
```

**Acceptance:**

- Follow-up questions in same conversation avoid repeated scope reconstruction.
- No cross-conversation or cross-user leakage.
- Cache miss falls back to full-quality retrieval.

### Task 2.3: Prepare `postgres_lexical` Grey Release

**Files:**
- Modify: `docs/validation/retrieval-quality-smoke.md`
- Modify: `scripts/run-retrieval-quality-smoke.sh`
- Modify: `crates/platform-api/src/lib.rs`
- Test: existing retrieval tests in `crates/platform-api/src/lib.rs`

**Implementation Steps:**

1. Add a side-by-side smoke mode:
   - run same cases through `legacy_scan`
   - run same cases through `postgres_lexical`
   - compare Recall@20, MRR@20, p95 retrieval latency, permission leaks
2. Add response metadata field for internal observability:
   - `retrieval_backend`
   - `candidate_count`
   - `deduped_candidate_count`
   - `retrieval_latency_ms`
3. Add an environment-only grey flag:
   - `RETRIEVAL_SEARCH_BACKEND=postgres_lexical`
   - rollback is `RETRIEVAL_SEARCH_BACKEND=legacy_scan` or unset.
4. Do not enable by default until live answer quality receipts pass.

**Validation Commands:**

```bash
cargo test -p platform-api postgres_lexical_retrieval_search_prefers_cjk_phrase_match --lib
cargo test -p platform-api postgres_lexical_retrieval_search_recalls_deep_old_chunk_beyond_latest_window --lib
bash scripts/run-retrieval-quality-smoke.sh --baseline
```

**Acceptance:**

- `postgres_lexical` has no recall or permission regression versus legacy.
- p95 retrieval latency improves or remains acceptable.
- One-command rollback documented before server flag change.

---

## Phase 3: Static Page and Codex Executor Throughput

### Task 3.1: Make Static Page Stages Explicit and Non-Blocking

**Files:**
- Modify: `crates/static-page-worker/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/components/ChatPanel.js`
- Modify: `apps/web/app/components/InsightPanel.js`
- Test: `crates/static-page-worker/src/main.rs`
- Test: `apps/web/app/lib/codex-customer-artifacts.test.mjs`

**Implementation Steps:**

1. Ensure each static page stage emits a user-visible but bounded event:
   - planning
   - image_prompt_ready
   - image2_submitted
   - image_preview_ready
   - html_generation_started
   - html_published
   - background_optimization_started
   - background_optimization_done
2. If local published URL is available, return it immediately while Codex optimization continues.
3. If Codex is queued, show queue state, not failure.
4. If Codex fails, keep the best available generated artifact and explain retry options.

**Validation Commands:**

```bash
cargo test -p static-page-worker static_page_image_task_payload_tracks_non_blocking_poll_state --lib
node --test apps/web/app/lib/codex-customer-artifacts.test.mjs
npm run test:static-page-export-artifact
```

**Acceptance:**

- Third-party and main-site users receive at least one progress event before long work.
- Public URL is returned once a valid artifact exists.
- No duplicate links in final reply.

### Task 3.2: Reuse Existing Artifact Baselines

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/components/static-page/StaticPageFinalRender.js`
- Test: `apps/web/app/lib/static-page-draft.test.mjs`

**Implementation Steps:**

1. Build a deterministic artifact baseline key:
   - tenant
   - dataset set
   - database source set
   - report intent
   - template id
   - output format
2. If user asks to modify or refresh, attach existing artifact context to the task.
3. If user explicitly asks to redesign or remake, bypass reuse.
4. Store reuse decision in draft metadata.

**Validation Commands:**

```bash
node --test apps/web/app/lib/static-page-draft.test.mjs
npm run smoke:static-page-5way -- --self-test
```

**Acceptance:**

- Existing approved pages are reused for refresh/modify.
- New design is only triggered by explicit redesign intent.
- Reuse does not skip data refresh.

### Task 3.3: Codex Executor Queue Controls

**Files:**
- Modify: `crates/static-page-worker/src/main.rs`
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `docs/operations/static-page-image2-data-publish.md`
- Test: `crates/static-page-worker/src/main.rs`

**Implementation Steps:**

1. Add per-task-kind concurrency caps:
   - image preview
   - local HTML render
   - Codex executor publish
   - silent prewarm
2. Add backoff for transient provider load errors.
3. Deduplicate identical queued publish tasks by artifact baseline key.
4. Prevent silent prewarm from competing with customer-visible jobs.

**Validation Commands:**

```bash
cargo test -p static-page-worker parse_static_page_worker_concurrency_defaults_and_clamps --lib
cargo test -p static-page-worker static_page_image_auto_retry_classifies_transient_errors --lib
```

**Acceptance:**

- Customer-visible jobs are prioritized.
- Duplicate publish jobs collapse to one active task.
- Transient provider failures retry without returning false final failure.

---

## Phase 4: Database Source Performance and Structured Query Plans

### Task 4.1: Persist Database Source Profiles

**Files:**
- Modify: `crates/storage/migrations/0017_database_source_profiles.sql`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/external-source-worker/src/main.rs`
- Test: `crates/platform-api/src/lib.rs`

**Implementation Steps:**

1. Persist per database source:
   - table list
   - row counts
   - time columns
   - dimension columns
   - metric columns
   - enum/high-cardinality hints
   - latest sync timestamp
2. Refresh profile after database sync.
3. Feed profile summary to assistant without scanning raw rows.

**Validation Commands:**

```bash
cargo test -p platform-api database_source_profile --lib
cargo test -p external-source-worker database_source_profile --lib
```

**Acceptance:**

- DataMax can answer schema/field capability questions from profile.
- Profile refresh does not block normal chat.

### Task 4.2: Add Structured Query Plan for Numeric Questions

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Implementation Steps:**

1. Classify questions that require structured data:
   - TopN/ranking
   - trend
   - comparison
   - sum/average/count
   - time range filter
   - store/region/category dimension
2. Generate a constrained internal query plan from database profile, not free-form model SQL.
3. Execute only allowlisted aggregate queries.
4. Feed aggregate result plus narrative evidence to the model.
5. If structured query cannot be planned, fall back to current evidence supply with a visible limitation note.

**Validation Commands:**

```bash
cargo test -p platform-api structured_database_query_plan --lib
cargo test -p assistant-runtime database_query_intent --lib
npm run smoke:newbai-customer-answer -- --self-test
```

**Acceptance:**

- Numeric answers use structured aggregates.
- Model does not guess numbers from chunks.
- Fallback preserves answer quality and cites limitation.

### Task 4.3: Precompute Common Database Aggregates

**Files:**
- Modify: `crates/dataset-output-worker/src/main.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/migrations/0018_database_aggregate_cache.sql`
- Test: `crates/dataset-output-worker/src/main.rs`

**Implementation Steps:**

1. Cache common aggregates per synced database dataset:
   - latest month
   - previous month
   - TopN by primary metric
   - trend by time
   - dimension distribution
2. Expire cache on new sync run.
3. Use cache in report/static-page planning before querying detail rows.

**Validation Commands:**

```bash
cargo test -p dataset-output-worker database_aggregate_cache --lib
cargo test -p platform-api static_page_uses_database_aggregate_cache --lib
```

**Acceptance:**

- Report planning receives real aggregate data quickly.
- Detail rows remain available for drilldown and validation.

---

## Phase 5: Parsing and Indexing Throughput

### Task 5.1: Quick Parse Then Detailed Parse

**Files:**
- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/retrieval-worker/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/ingest-worker/src/main.rs`

**Implementation Steps:**

1. Split parse states:
   - `uploaded`
   - `quick_parsed`
   - `indexed_basic`
   - `detailed_parsing`
   - `indexed_detailed`
   - `vlm_repaired`
2. Allow chat to use quick/basic index while detailed parsing continues.
3. Surface parse status to the model and user so answers can say what is available.
4. Do not mark detailed parsing slow work as failure unless terminal error occurs.

**Validation Commands:**

```bash
cargo test -p ingest-worker quick_parse_state --lib
cargo test -p platform-api assistant_uses_parse_status_supply --lib
```

**Acceptance:**

- Uploaded document becomes minimally queryable faster.
- Later detailed parsing improves answers automatically.
- Slow VLM does not block ordinary chat.

### Task 5.2: Fingerprint-Based OCR/VLM Deduplication

**Files:**
- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/document-vlm-runtime/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Test: `crates/ingest-worker/src/main.rs`

**Implementation Steps:**

1. Use existing document fingerprint/canonical enrichment records to detect duplicate content.
2. Reuse OCR/VLM/page parse artifacts when document bytes and parser version match.
3. Re-run detailed parse only when:
   - file fingerprint changes
   - parser version changes
   - quality gate requests repair
4. Preserve tenant/document ACL separately; reuse content artifacts without making documents public.

**Validation Commands:**

```bash
cargo test -p ingest-worker reuses_parse_artifacts_for_duplicate_fingerprint --lib
cargo test -p platform-api duplicate_private_document_acl_not_leaked --lib
```

**Acceptance:**

- Duplicate uploads do not repeat OCR/VLM.
- ACL and dataset membership remain isolated.

---

## Phase 6: Frontend and Observability Page Performance

### Task 6.1: Lazy Load Heavy Panels

**Files:**
- Modify: `apps/web/app/admin/external-integrations/page.js`
- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/InsightPanel.js`
- Test: `apps/web/app/lib/external-integrations.test.mjs`

**Implementation Steps:**

1. Load queue stats only when queue panel is selected.
2. Load conversation test detail only when a specific conversation is selected.
3. Load static-page iframe/large preview only when opened.
4. Avoid permanent polling when tab/panel is hidden.

**Validation Commands:**

```bash
node --test apps/web/app/lib/external-integrations.test.mjs
```

**Acceptance:**

- Observability page does not create background load when idle.
- Selecting one object only loads that object's detail.

### Task 6.2: Cap Rendered Chat Trace Size

**Files:**
- Modify: `apps/web/app/components/ChatPanel.js`
- Modify: `apps/web/app/lib/html-artifact-manifest.js`
- Test: `apps/web/app/lib/html-artifact-manifest.test.mjs`

**Implementation Steps:**

1. Render user-facing text normally.
2. Collapse long JSON/trace blocks behind "view details" in admin-only surfaces.
3. In user chat, reject internal trace payloads and repeated artifact links.
4. Keep full trace export available as protected downloaded JSON for operators.

**Validation Commands:**

```bash
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
npm run smoke:external-channel-20way -- --self-test
```

**Acceptance:**

- No giant internal JSON in chat.
- No repeated links.
- Operator can still inspect protected trace when needed.

---

## Phase 7: Release, Grey Rollout, and Rollback

### Task 7.1: Local Verification Gate

**Commands:**

```bash
cargo fmt --check
cargo check -p platform-api
cargo check -p ingest-worker
cargo check -p retrieval-worker
cargo check -p static-page-worker
cargo check -p assistant-run-worker
node --test apps/web/app/lib/external-integrations.test.mjs
node --test apps/web/app/lib/static-page-draft.test.mjs
npm run check:pure-third-party-guide-html
bash scripts/run-retrieval-quality-smoke.sh --baseline
```

**Acceptance:**

- All checks pass.
- Existing unrelated third-party doc HTML changes are either explicitly included by request or left untouched.

### Task 7.2: 8 Server Read-Only Preflight

**Commands:**

```bash
ssh root@8.155.8.7 'cd /srv/aiv3/repo && git status --short --branch && git rev-parse --short HEAD'
ssh root@8.155.8.7 'systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-assistant-run-worker.service aiv3-ingest-worker.service aiv3-retrieval-worker.service aiv3-static-page-worker.service aiv3-media-worker.service'
ssh root@8.155.8.7 'curl -fsS http://127.0.0.1:3000/healthz && echo && curl -fsS http://127.0.0.1:3000/readyz'
```

**Acceptance:**

- Server is healthy before deployment.
- Current server commit and rollback point are recorded.

### Task 7.3: Deployment Gate

Deployment requires explicit user approval.

**Commands after approval:**

```bash
ssh root@8.155.8.7 'cd /srv/aiv3/repo && git pull --ff-only'
ssh root@8.155.8.7 'cd /srv/aiv3/repo && CC=clang CXX=clang++ cargo build --release -p platform-api -p ingest-worker -p retrieval-worker -p assistant-run-worker -p static-page-worker'
ssh root@8.155.8.7 'systemctl restart aiv3-platform-api.service aiv3-ingest-worker.service aiv3-retrieval-worker.service aiv3-assistant-run-worker.service aiv3-static-page-worker.service'
```

**Post-Deploy Smokes:**

```bash
npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com
npm run smoke:external-channel-streaming-10way -- --base-url https://v3.elepcloud.com
npm run smoke:static-page-5way -- --base-url https://v3.elepcloud.com
bash scripts/run-retrieval-quality-smoke.sh --base-url https://v3.elepcloud.com --live-subset
```

**Acceptance:**

- p95 does not regress from baseline.
- Recall/MRR and permission leak gates pass.
- Static page returns a valid artifact link.
- Third-party streaming returns progress and final result without duplicate links.

### Task 7.4: Rollback

**Rollback Conditions:**

- Permission leak count > 0.
- Retrieval Recall@20 or MRR@20 regresses on required cases.
- Ordinary chat p95 worsens materially without quality gain.
- Static page jobs fail at higher rate or stop returning public URL.
- Third-party replies emit giant internal JSON or repeated links.

**Rollback Actions:**

1. Set `RETRIEVAL_SEARCH_BACKEND=legacy_scan` or remove the flag.
2. Disable new caches through env flag if cache correctness is suspected.
3. Lower new worker concurrency caps to previous values.
4. Revert to previous Git commit only if code-level rollback is required.
5. Record rollback reason in `docs/validation/system-performance-baseline.md`.

---

## Recommended Execution Order

1. Phase 1: Observability baseline.
2. Phase 2.1: Parallel context preparation.
3. Phase 2.3: `postgres_lexical` side-by-side comparison and grey readiness.
4. Phase 3.1: Static page progress and non-blocking delivery.
5. Phase 3.2: Artifact reuse for report modification.
6. Phase 4: Database source structured query plan.
7. Phase 5: Quick parse plus detailed parse.
8. Phase 6: Frontend lazy loading and trace caps.
9. Phase 7: Controlled deploy and rollback.

## First Development Slice

Start with Phase 1 only.

Reason:

- It does not change answer behavior.
- It creates the evidence needed to prove later optimizations.
- It protects quality by making regressions visible.

First slice commit target:

```bash
git add docs/plans/2026-06-12-v3-system-performance-optimization.md \
  crates/storage/migrations/0015_assistant_run_performance_events.sql \
  crates/storage/src/lib.rs \
  crates/platform-api/src/lib.rs \
  scripts/smoke/system-performance-baseline.mjs \
  docs/validation/system-performance-baseline.md \
  package.json
git commit -m "Add V3 performance baseline instrumentation"
```

Do not include unrelated integration-guide HTML changes unless the user explicitly asks to collect those docs in the same release.

