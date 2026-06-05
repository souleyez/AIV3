# Static Page Image-First Pipeline Optimization Plan

**Date:** 2026-05-30

**Goal:** Make the static-page and image-generation chain stable, observable, and faster where it can be faster, while keeping the product rule that high-quality static pages must start from a generated visual image.

**Primary repos:**

- `ai-data-platform-v3`: system of record, third-party assistant, static-page image jobs, workflow tasks, status snapshots, final artifact publishing.
- `codex-web`: Codex orchestrator, Cloudflare Codex runtime target, image artifacts, fixed Codex execution, runtime usage and queue policy.

**Core product decision:** The formal high-quality static-page path remains image-first. DataMax should generate a visual/effect image first, persist it, and then ask Cloudflare Codex to generate the final static page from the visual plus structured data. A fast direct renderer may exist only as a fallback preview or low-requirement draft path. It must not replace the formal image-first delivery path.

**Progress update 2026-05-30:** P0 status backfill has started in `crates/platform-api/src/lib.rs`. Static-page publish rejection now writes `assistant_run.external_channel_static_page_publish_failed`, status replies prefer that business event over the raw fixed-task event, and targeted tests cover failed-event priority plus failed-event append/dedupe.

**Progress update 2026-05-30 P1 slice:** Static-page visual generation and Codex Host final publish now use non-blocking remote polling slices. The worker submits or polls once, persists the remote task id/poll state into the workflow task payload, requeues itself with `available_at`, and releases the local worker while the Cloudflare/Codex task continues. Status replies now surface `static_page_image_job.submitted/running/poll_retry` as visible image-generation progress.

**Progress update 2026-05-30 P1/P2 queue metadata slice:** Workflow task views now expose `logical_queue`, `logical_task_key`, `remote_task_id`, and `next_poll_at` from durable task payloads. Static-page image preview and final static-page publish tasks persist logical poll metadata such as `static_page_image_preview` / `poll_static_page_image_preview` and `static_page_publish` / `poll_static_page_publish`, while leaving the physical database `task_key` unchanged for compatibility with existing workflow stages and worker filters.

**Progress update 2026-05-30 P2 stats slice:** Added a lightweight visible-workflow queue stats surface at `/v1/workflow-tasks/queue-stats`. It summarizes recent visible workflow tasks by logical queue and logical task key, including queued/running/retrying/succeeded/failed/cancelled/dead-lettered counts and the next queued `available_at`, so the UI and operators can distinguish effect-image generation from final static-page publish without changing the physical worker queues.

**Progress update 2026-05-30 P2 UI slice:** The external integrations Codex executor panel now loads `/api/v3/external/codex-executor-tasks/queue-stats` on demand and shows a queue snapshot above the task list. The web helper layer normalizes logical queue/task-key counts and treats submitted/pending remote Cloudflare tasks as active poll states, so operators can see whether time is being spent in effect-image generation, final page publishing, or generic Codex fixed tasks.

**Progress update 2026-05-30 P4 asset provenance slice:** Static-page effect images now keep a stable DataMax preview asset as the render-facing URL and attach a safe provenance summary to the image job payload and preview-ready artifact manifest. The provenance records the source asset kind, redacted source reference, persisted preview URL, byte size, mime type, dimensions, storage status, and orchestrator task id without leaking embedded data URLs or signed query tokens.

**Progress update 2026-05-30 P4 fixed-task handoff slice:** The third-party `static_page_image2_data_publish` fixed-task context now includes `render_asset_url` and a compact `asset_provenance` block for Codex Host. Platform API and Codex Host both strip signed query strings and embedded image refs before the final executor prompt is built, and `prompt_text` is sourced from the safe Image2 summary instead of the raw payload.

**Progress update 2026-05-30 P3 service-call slice:** Codex Host orchestrator submit/poll requests now use a shared service-header helper that sends `Authorization`, `X-Client-Name`, and explicit `User-Agent` headers, with optional JSON and idempotency headers for submit. Static-page and Codex Host retry classification now treats HTTP 500/502/503/504 as transient, while 401/403 auth failures and Cloudflare 1010/browser-signature blocks are surfaced as operator-visible infrastructure/configuration errors instead of endless user-facing retries.

**Progress update 2026-05-31 smoke-readiness slice:** The Cloudflare Codex fixed-task smoke remote mode now checks real public/read-only surfaces instead of following `/healthz` frontend redirects. The guarded readiness check validates the public third-party guide, missing-token external events auth guard, and workflow queue JSON diagnostics before marking 8-server readiness as passed.

**Progress update 2026-05-31 P2 latency slice:** Workflow queue stats now include finished-task duration P50/P95 at both logical queue and logical task-key level, so operators can distinguish count/backlog from actual completed runtime when comparing effect-image preview, final static-page publish, and other workflow queues.

**Progress update 2026-05-31 P2 UI latency slice:** The external integrations Codex executor panel now normalizes and displays the queue/task-key P50/P95 runtime metrics from workflow queue stats, so the lazy-loaded observability view shows both active backlog and recent completed runtime without opening individual task details.

**Progress update 2026-05-31 P2 success-latency slice:** Workflow queue stats now also expose success-only P50/P95 runtime metrics, and the observability page prefers those values when present. This keeps old failed/stale tasks from making the normal Image2/Codex runtime look worse than successful jobs.

**Progress update 2026-05-31 P0 auto-repair slice:** Static-page preview/final-render entry points now refresh the DataMax data contract before gating or rendering, including third-party supplied image prompt payloads. Database aggregate rows are carried into field-candidate sample data, and chart `dataKey` / binding-quality field paths are treated as repairable bindings so DataMax can generate a page first and keep data warnings/adjustment paths visible instead of stopping on `needs_sample_rows`.

**Progress update 2026-05-31 smoke-mutation-readiness slice:** `run-cloudflare-codex-fixed-task-smoke.ps1` now supports a reviewed real `static-page-no-confirm` third-party mutation smoke with private bearer/config input, polling of the returned status URL, and redacted artifact/status reporting. The guard was verified against 8服务器 without a bearer: the script fails before mutation with `mutation_attempted=false`.

**Progress update 2026-05-31 data-ingestion smoke-readiness slice:** The same fixed-task smoke now supports reviewed real `data-ingestion-analysis` mutation checks. It requires both a private bearer and a private source selector before mutation, then polls the assistant-run reply status URL for queued/running/completed/needs-human states without exposing credentials or source dumps.

**Progress update 2026-05-31 real smoke slice:** The real 8-server third-party static-page mutation smoke passed and returned a generated-artifact URL immediately. The data-ingestion mutation smoke reached real execution after adding `data_ingestion_analysis` to the platform and Codex Host agent allowlists; it now reports queued/running/retrying instead of auth/source/allowlist failure, with terminal polling still outstanding.

**Progress update 2026-06-02 template-library visibility slice:** DataMax now surfaces generated static-page template reuse decisions in third-party status cards and final replies. Exact dataset-combination reuse reports `template_match_policy=exact_dataset_artifact_key`; relaxed dataset-overlap reuse reports `template_match_policy=dataset_overlap` plus `relaxed_template_match`; normal explicit/inferred templates report `explicit_or_inferred_template`. The same fields are preserved when status replies are reconstructed from AssistantRun events, and the pure/full third-party integration docs now describe the template-library reuse policy and response fields.

**Progress update 2026-06-05 Xinbai default-template convergence:** The Xinbai report template library has been narrowed to one accepted default: `xinbai-functional-modular-template-20260604`. Historical Xinbai pages remain as published artifacts but are not accepted defaults. The accepted monthly report template uses refreshed same-directory `data.json`, a compact time/range filter, one summary KPI card, a right-middle customer detail panel without extra explanatory header copy, and prompt-focused reuse links such as `?focus=风险店铺`, `?focus=取高机会`, `?focus=低活跃`, and `?focus=品牌明细`.

---

## Baseline From 2026-05-30 Inspection

Observed from 8 server PostgreSQL events, worker logs, and 1 server orchestrator HTTPS API:

- Static HTML rendering itself is not the bottleneck.
  - `static_page_render` succeeded in about `0.9s`.
  - Failed render validation also returned in about `0.1s`.
- Image/effect visual generation is the first real bottleneck.
  - 8-side successful `static_page_image_jobs`: average about `300s`, P50 about `226s`, P95 about `606s`.
  - 1-side orchestrator `static-page-visual`: P50 about `214s`, P95 about `294s`.
  - 1-side orchestrator `image-draft`: P50 about `223s`, P95 about `384s`.
- Final Cloudflare Codex static-page execution is the second bottleneck.
  - Successful `codex_host_task_workflow`: P50 about `142s`, P95 about `552s`.
  - Failed tasks include 500/503 polling failures and old `900000ms` timeout behavior.
- Wake/submit overhead is small.
  - `runtimeWakeToSubmitMs` is usually about `8-10s`.
  - `submitToFirstPollMs` is usually `0-3s`.
  - Artifact backfill is usually a few seconds.
- Queue blocking was previously a major risk.
  - Old stale claimed `static_page` tasks have been cleaned and automatic stale cleanup is deployed.
  - Current snapshot after cleanup showed no queued/running stuck tasks in `workflow_tasks` for `static_page`/`codex_host`.
- Status accuracy is still a major risk.
  - Some runs reach `static_page_image_job.preview_ready`, then Codex publish fails or stalls.
  - The assistant/user-facing status can remain effectively "processing" because publish failure is not always written back as a clear `publish_failed` state.
- Cloudflare WAF can affect service-to-service calls.
  - A generic Python user agent received Cloudflare `1010 browser_signature_banned`.
  - Explicit service/curl/browser user agents succeeded.
  - Existing worker paths already set service user agents in key places, but orchestrator service routes should still be explicitly allowed.

---

## Target User-Visible Flow

```text
User / third-party assistant request
  -> DataMax creates assistant_run and static_page_draft
  -> DataMax queues image_preview job
  -> 1 server / Cloudflare Codex generates visual image
  -> DataMax persists preview asset and marks preview_ready
  -> DataMax queues static_page_publish job
  -> 1 server / Cloudflare Codex generates final HTML from visual + uiSpec/modelOutput/data snapshot
  -> DataMax validates and publishes generated artifact
  -> User sees final URL, with accurate status snapshots throughout
```

Important behavior:

- Once a task is accepted, it should not look dead just because it is slow.
- A task may wait for a long time, but it must keep an accurate status snapshot.
- Only true terminal failures should show failure/refund/retry UI.
- Slow/pending states should stay pending, not become fake "failed" states.

---

## Status Model

Use a unified status vocabulary across DataMax, mini program/web clients, and `codex-web`.

| Stage | Internal examples | User-facing meaning |
| --- | --- | --- |
| `accepted` | request received, draft created | 已提交 |
| `image_queued` | `static_page_image_job.created` | 效果图排队中 |
| `image_running` | orchestrator submitted/running/heartbeat | 效果图生成中 |
| `image_ready` | `static_page_image_job.preview_ready` | 效果图已生成 |
| `publish_queued` | `assistant_run.external_channel_static_page_publish_queued` | 页面生成排队中 |
| `publish_running` | `codex_host_task.cloudflare_heartbeat` / `exec_heartbeat` | 页面生成中 |
| `published` | `assistant_run.external_channel_static_page_publish_completed` | 页面已生成 |
| `retrying` | poll retry / transient 500/503 / lease retry | 临时异常，继续重试 |
| `failed` | true terminal failure | 生成失败，可重试/退款 |
| `cancelled` | user/system cancellation | 已取消 |

Processing statuses that must not be treated as terminal failure:

```text
queued
submitted
running
pending
processing
in_progress
retry_wait
retrying
waking_runtime
waiting
poll_retry
```

Terminal failure should require one of:

- Cloudflare orchestrator task explicitly returns `failed`/`cancelled`.
- DataMax validation rejects a completed artifact.
- User cancels.
- Stale lease cleanup expires an abandoned local worker task.

---

## Architecture Direction

### 1. Keep Image-First Static Page Quality Path

Formal static-page generation keeps this dependency:

```text
uiSpec/modelOutput/data snapshot
  -> effect image
  -> final HTML generation from effect image and structured contract
```

`uiSpec` / `modelOutput` should contain:

- title and objective;
- content modules;
- image slots and selected effect image references;
- button/link requirements;
- style tokens;
- mobile layout intent;
- data bindings or evidence references;
- delivery rules and validation constraints.

The effect image is a first-class artifact, not just a temporary prompt aid.

### 1A. Reuse Accepted Templates Before Redesign

The template library should prefer a stable accepted template when the current dataset/domain overlaps an existing accepted template. Reuse means:

- keep the published page URL stable when the customer is adjusting or querying the same report family;
- refresh or replace `data.json` / `data-snapshot.json` from the current dataset scope;
- pass conversation emphasis through URL/query metadata such as `focus` so the relevant module appears first;
- avoid re-running Image2/Codex for style unless the user explicitly asks for a new visual direction, the accepted template is missing, or the data contract is incompatible.

For Xinbai, the only accepted default template is the modular monthly report. Old dark/fallback/one-off Xinbai pages may remain visible as artifacts, but they must not compete as default templates.

### 2. Make Worker Polling Non-Blocking

Current risk: one worker can submit a remote task and then hold the local workflow execution while polling the remote runtime.

Target:

```text
submit_remote_task
  -> persist remote task id
  -> finish current local task quickly
  -> enqueue poll_remote_task with available_at
  -> poll until terminal
  -> backfill final state
```

Expected effect:

- Multiple jobs no longer block behind one long remote task.
- Worker restarts are safer because the remote task id is durable.
- Slow Cloudflare tasks become "tracked pending" instead of occupying a local worker slot.

### 3. Split Logical Queues

Suggested logical queues:

| Queue | Purpose |
| --- | --- |
| `static_page_image_preview` | Effect image generation for static pages |
| `static_page_publish` | Final HTML/page generation after image ready |
| `product_image_generation` | Goods editor / mini program product images |
| `codex_fixed_task` | General fixed Codex tasks |

The physical implementation can still use existing `workflow_tasks` first. The important change is separate task keys, status counters, and user-visible estimates.

### 4. Stabilize Cloudflare Service Calls

`https://souleye.cc/api/codex/orchestrator/*` should be treated as service-to-service API traffic.

Requirements:

- Send explicit `User-Agent` and `X-Client-Name` from every DataMax caller.
- Add Cloudflare WAF/Access rules that allow known service calls from 8 server or trusted service keys.
- Do not rely on browser-signature behavior for server jobs.
- Treat 500/503 and empty/invalid JSON as transient unless repeated past retry policy.

### 5. Persist Images Once, Use Stable URLs

When the effect image is ready:

- Persist it in DataMax generated-artifacts/static-page preview storage.
- Store stable `preview_asset_key`.
- Avoid passing temporary or expiring remote image URLs into final Codex generation.
- Continue to keep retention generous on 1 server until storage reaches the configured threshold.

---

## Implementation Phases

## P0: Status Accuracy And Failure Backfill

**Objective:** Stop fake "stuck" behavior by making every slow/fail path visible and accurate.

Tasks:

1. Add explicit publish failure event.
   - When `codex_host_task_workflow` fails after `assistant_run.external_channel_static_page_publish_queued`, append:
     - `assistant_run.external_channel_static_page_publish_failed`
     - payload with `codex_host_workflow_execution_id`, safe error code, retryability, elapsed time, and related image job/draft ids.
   - Preserve existing local fallback HTML only as fallback, not as final success.

2. Normalize status responses.
   - `GET /assistant-runs/{id}/reply` and external status endpoints should map events into the unified status model above.
   - `preview_ready_only` should become a clear "effect image ready, final page pending/running/failed" state.

3. Keep transient polling non-terminal.
   - 500/503, empty body, JSON decode failures, and short network errors write `poll_retry` or `retrying`.
   - They should not immediately mark final failure unless retry policy is exhausted.

4. Add status tests.
   - Preview ready + no publish yet -> `publish_queued` or `publish_running`.
   - Preview ready + Codex failure -> `publish_failed`.
   - Preview ready + Codex completed -> `published`.
   - Transient poll error -> `retrying`.

Files likely involved:

- `crates/platform-api/src/lib.rs`
- `crates/codex-host-agent/src/main.rs`
- `crates/static-page-worker/src/main.rs`
- `crates/contracts/src/lib.rs`
- external integration docs under `docs/integrations/`

Done when:

- A real failed publish no longer looks like endless processing.
- User-facing status can explain which stage is slow.

## P1: Non-Blocking Remote Task Polling

**Objective:** Local workers should not be occupied while remote Cloudflare tasks run for minutes.

Tasks:

1. Introduce durable remote task tracking.
   - Persist remote orchestrator task id in workflow context/task payload.
   - Store `remote_submitted_at`, `last_poll_at`, `poll_attempt`, and `next_poll_at`.

2. Split task keys.
   - `submit_static_page_image_preview`
   - `poll_static_page_image_preview`
   - `submit_static_page_publish`
   - `poll_static_page_publish`

3. Make submit tasks finish quickly.
   - Submit remote task.
   - Persist task id.
   - Enqueue poll task with `available_at`.
   - Release worker.

4. Make poll tasks idempotent.
   - If remote task is still running, requeue poll with backoff.
   - If remote task completed, import artifact and mark next local state.
   - If remote task failed/cancelled, write clear terminal status.

5. Add lease/stale cleanup compatibility.
   - Local poll tasks may expire and retry safely because remote task id is durable.
   - Do not create duplicate remote tasks after restart.

Done when:

- One slow Cloudflare task does not block other static-page or product-image tasks.
- Worker restart can resume polling the same remote task.

## P2: Queue Split And Estimates

**Objective:** Separate workloads so product image jobs, effect images, and final page generation do not hide each other.

Tasks:

1. Add logical queue/task stats.
   - Count queued/running/retrying/failed/succeeded by queue and task key.
   - Expose P50/P95 from recent events where available.

2. Update user-facing estimates.
   - Mini program and web should show estimates by queue type.
   - Static page should distinguish "effect image" time from "final page" time.

3. Add queue policy.
   - Static page image preview and static page publish can have separate concurrency/limits.
   - Product image generation should not starve static page publish.

4. Keep refund/failure UI subdued.
   - Only true `failed` states show refund/retry affordances.
   - Pending/retrying states stay quiet.

Done when:

- A product-image spike does not make static-page final publish look broken.
- Queue estimates describe the current stage, not a single vague wait time.

## P3: Cloudflare API Allowlist And Retry Policy

**Objective:** Make 8 -> 1 service calls reliable and predictable.

Tasks:

1. Ensure all DataMax orchestrator requests include:

```text
User-Agent: AIDataPlatformV3StaticPageWorker/1.0 or DataMax-codex-host-agent
X-Client-Name: ai-data-platform-static-pages or DataMax-codex-host-agent
Authorization: Bearer <service key>
```

2. Configure Cloudflare rule for orchestrator API.
   - Scope path: `/api/codex/orchestrator/*`
   - Allow trusted 8 server IP/service traffic and valid service keys.
   - Avoid browser-signature challenges for service API calls.

3. Add smoke checks.
   - From 8 server using worker UA.
   - From 8 server using codex-host-agent UA.
   - Confirm no 1010/WAF challenge.

4. Retry policy.
   - 500/503: retry with backoff.
   - 403 Cloudflare 1010: infrastructure/config failure, not user failure.
   - 401/403 auth failure from app: credential failure, requires operator action.

Done when:

- 8 server service calls do not depend on browser-like behavior.
- WAF/config failures are reported as operator-visible infrastructure errors.

## P4: Stable Asset Storage And Retention

**Objective:** Prevent 401/404 temporary-image problems and avoid unnecessary image cleanup during active work.

Tasks:

1. Import every successful effect image into DataMax stable storage.
2. Use the stable DataMax preview URL for final Codex generation.
3. Track image provenance:
   - remote orchestrator task id;
   - original artifact URL;
   - persisted DataMax asset key;
   - mime type and size;
   - created time.
4. Align retention:
   - 1 server can keep generated images until storage reaches configured threshold.
   - DataMax should retain artifacts needed by active assistant runs and published pages.

Done when:

- Final page generation does not fail because an upstream image URL expired.
- A published static page can be audited back to its effect image.

## P5: Observability Dashboard

**Objective:** Make future optimization measurable instead of anecdotal.

Metrics to expose:

- image preview queue wait;
- image preview runtime;
- image preview import time;
- publish queue wait;
- publish runtime;
- artifact validation time;
- total request-to-final-page time;
- failures by code;
- transient retry count;
- current queue depth by queue;
- P50/P95 by queue and task kind.

Data sources:

- `assistant_run_events`
- `workflow_tasks`
- `workflow_executions`
- `static_page_image_jobs`
- `codex-web` orchestrator task `phaseDurationsMs`

Suggested surfaces:

- operator-only DataMax page;
- lightweight CLI/report query first;
- later add admin UI.

Done when:

- Before/after optimization can be compared by P50/P95.
- A bad day can be diagnosed by queue, runtime, provider, or Cloudflare errors.

## P6: Optional Multi-Executor Scale-Out

**Objective:** Add throughput only after the status and queue model is stable.

Options:

1. Add a second Cloudflare Codex runtime target.
2. Dedicate one runtime to image tasks and one to final static-page tasks.
3. Add provider-backed image generation outside Codex only if product quality and tool access are acceptable.

Rules:

- Do not add concurrency before P0/P1 are stable.
- Per-user/per-tenant fairness should be enforced before opening wide concurrency.
- Multi-executor should improve throughput, not hide broken task status.

---

## Testing Plan

Unit tests:

- event-to-status mapping;
- retryable vs terminal error classification;
- durable remote task id extraction;
- duplicate submit prevention after worker restart;
- queue position and estimate calculation.

Integration tests:

- submit image preview -> remote task id persisted -> poll -> preview ready;
- preview ready -> submit publish -> remote task id persisted -> poll -> published;
- preview ready -> publish failed -> `publish_failed` status;
- remote 503 -> retry event, not terminal failure;
- remote task still running after worker restart -> resumed polling.

Live smoke:

1. From 8 server, submit static-page request through external channel.
2. Confirm `static_page_image_job.created`.
3. Confirm 1 server orchestrator task exists.
4. Confirm `preview_ready`.
5. Confirm `assistant_run.external_channel_static_page_publish_queued`.
6. Confirm final generated artifact URL returns HTTP 200.
7. Confirm status endpoint moves through accurate stages.
8. Confirm metrics show phase durations.

---

## Rollout Order

1. P0 status accuracy and publish failure backfill.
2. P3 Cloudflare service-call allowlist and retry classification.
3. P1 non-blocking polling for image preview.
4. P1 non-blocking polling for static page publish.
5. P2 logical queue split and estimates.
6. P4 stable image asset storage audit fields.
7. P5 observability dashboard.
8. P6 second runtime or multi-executor scale-out.

This order prioritizes "do not look stuck" and "do not block the queue" before raw speed. It also preserves the image-first quality requirement.

---

## Open Questions

1. Should every `preview_ready` static page automatically enter final publish, or should some channels require user confirmation first?
2. What is the default maximum wait shown to users for formal high-quality static pages: 10 minutes, 20 minutes, or "will notify when done"?
3. Should effect images be retained as part of the published page artifact bundle, or only as audit/source assets?
4. Should product images and static-page effect images share the same Cloudflare image queue long term, or be split at the runtime target level?
5. Which admin surface should own the observability dashboard: DataMax main admin, external integrations page, or `codex-web` admin?

---

## Success Criteria

- Formal static pages still use the image-first path.
- Slow tasks keep accurate status snapshots instead of appearing stuck.
- One slow Cloudflare task does not occupy the local static-page worker indefinitely.
- Transient 500/503/WAF issues are visible as retrying or infrastructure errors.
- Final HTML generation has clear P50/P95 metrics.
- Effect images are stable assets before final page generation begins.
- Future multi-executor scaling has a clean queue/status foundation.
