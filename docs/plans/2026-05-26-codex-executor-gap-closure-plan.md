# Codex Executor Gap Closure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finish the Codex executor integration after the first queue/agent/observability bridge by closing business-result feedback, safety, retry, and smoke gaps.

**Architecture:** V3 remains the system of record for task enqueue, permissions, datasets, artifacts, and third-party responses. Codex Host may execute fixed templates only after platform and agent readiness gates pass, and every result must return through V3 validation, AssistantRun events, artifact manifests, or staged import plans.

**Tech Stack:** Rust `platform-api`, `contracts`, `workflow-definitions`, `codex-host-agent`; PostgreSQL workflow/AssistantRun events; Next.js external integration observability page; generated third-party integration docs; 8-server smoke scripts.

---

## Current State

- `CodexHostTask` workflow and `codex_host/run_codex_host_task` queue exist.
- `crates/codex-host-agent` supports `dry_run`, `plan_only`, gated local-host `codex_exec`, and `cloudflare_orchestrator` for the fixed Cloudflare Codex executor.
- Fixed templates exist for `static_page_image2_data_publish`, `answer_quality_autofix`, and `data_ingestion_analysis`.
- Static-page auto publish now accepts either old `codex_exec` readiness or the fixed `cloudflare_orchestrator + cloudflare_codex` readiness; otherwise V3 returns direct HTML fallback with `render_output_id`.
- Integrated observability page has a protected Codex executor task panel; it only loads the task list when opened and only calls runtime inspect after selecting a task.

## Remaining Gaps

1. Static-page Codex publish still needs a real end-to-end smoke from effect image to generated artifact on an approved host.
2. `data_ingestion_analysis` validates output, but its successful result is not yet fully converted into a user-visible dataset/data-source staging workflow.
3. `answer_quality_autofix` can be represented as a fixed task, but the auto-fix loop still needs strict failure classification, patch review, and test-result feedback.
4. Failed/retrying executor statuses are recorded internally, but model-facing supply and third-party/operator-facing summaries are not yet uniform across all fixed templates.
5. Artifact manifests are not yet fully shared across static pages, Image2 previews, reports, data-ingestion plans, and generated artifacts.
6. Timeouts, retries, cancellation, concurrency budget, workspace retention, and cleanup policy need explicit operator rules and smoke coverage.
7. Real execution deployment is still blocked until host validation, credentials isolation, and task workspace behavior are proven on the approved executor host.

---

### Task 1: Freeze The Executor Readiness Matrix

**Files:**
- Modify: `docs/operations/codex-host-model-profiles.md`
- Modify: `docs/operations/codex-jump-host-minimax-smoke.md`
- Modify: `docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md`

**Step 1: Document the readiness states**

Add a table for:
- `disabled`;
- `preflight_rejected`;
- `plan_only`;
- `codex_exec_not_validated`;
- `codex_exec_ready`.

Include the exact env gates:
- `CODEX_HOST_TASK_ENABLED`;
- `CODEX_HOST_TASK_ALLOWLIST`;
- `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES`;
- `CODEX_HOST_AGENT_EXECUTION_MODE`;
- `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC`;
- `CODEX_HOST_AGENT_HOST_KIND`;
- `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT`.

**Step 2: Add expected behavior per state**

Document expected behavior:
- static-page requests return direct HTML fallback unless ready;
- data-ingestion requests may queue only in safe read-only/staging modes;
- answer-quality tasks cannot patch outside allowlisted files;
- real execution requires approved host validation.

**Step 3: Verify docs**

Run:

```powershell
npm run check:pure-third-party-guide-html
```

Expected: pass.

---

### Task 2: Add A Uniform Fixed-Task Status Feed For The Model

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write tests for model-facing status summaries**

Add tests covering:
- queued fixed task;
- preflight rejected;
- retrying/failed;
- completed static page;
- completed data ingestion;
- answer-quality needs human.

Expected model-facing fields:
- `template_id`;
- `workflow_execution_id`;
- `status`;
- `reason`;
- `retryable`;
- `next_step`;
- `artifact_links`;
- `staging_spec_available`;
- `human_review_required`.

**Step 2: Implement a shared summary helper**

Create a helper near existing `codex_host_fixed_task_*` helpers:

```rust
fn codex_host_fixed_task_model_supply_summary(event_payload: &Value) -> Value
```

Keep raw prompt, provider logs, stdout/stderr, credentials, and full file dumps out of the summary.

**Step 3: Attach summaries to AssistantRun evidence**

When fixed task audit events are recorded, append a safe model-facing summary to the run event payload or evidence state so follow-up answers can say:
- still processing;
- failed and retrying;
- needs human confirmation;
- artifact is ready;
- staging analysis is ready.

**Step 4: Run tests**

Run:

```powershell
cargo test -p platform-api codex_host_fixed_task --lib
cargo test -p platform-api workflow_runtime_pretty_summaries --lib
```

Expected: pass.

---

### Task 3: Complete Static-Page Publish Callback

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Modify: `docs/integrations/third-party-integration-api.zh-CN.md`

**Step 1: Extend success tests**

Add or extend tests proving that `static_page_image2_data_publish` success:
- validates `artifact.public_url`;
- requires `validation_report`;
- attaches an output artifact to the AssistantRun;
- records `assistant_run.external_channel_static_page_publish_completed`;
- creates a third-party reply shape with `artifact_links[0]`.

**Step 2: Add failure/retry tests**

Add tests for:
- missing `public_url`;
- URL not under `/generated-artifacts/`;
- missing `validation_report`;
- `needs_human`;
- `failed`.

Expected: no customer-facing final artifact link; safe operator/event reason only.

**Step 3: Implement any missing status mapping**

Ensure third-party follow-up/status surfaces can distinguish:
- `static_page_rendered` direct HTML fallback;
- `static_page_image2_auto_publish_pending`;
- `static_page_published`;
- `static_page_publish_needs_human`;
- `static_page_publish_failed`.

**Step 4: Regenerate docs**

Run:

```powershell
npm run build:pure-third-party-guide-html
npm run test:pure-third-party-guide-html
npm run check:pure-third-party-guide-html
```

Expected: pass.

---

### Task 4: Finish Data-Ingestion Analysis Result Handoff

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`
- Test: `crates/contracts/src/lib.rs`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Modify: `docs/integrations/third-party-integration-api.zh-CN.md`

**Step 1: Write tests for accepted outputs**

Cover `data_ingestion_analysis` output statuses:
- `analysis_ready`;
- `staging_spec_ready`;
- `needs_human`;
- `failed`.

Expected accepted outputs must include:
- `source_summary`;
- `data_quality_report`;
- `validation_checks`;
- optional `mapping_plan`;
- optional `staging_spec`.

**Step 2: Add AssistantRun event mapping**

On accepted output, record:
- `assistant_run.data_ingestion_analysis_completed`;
- `assistant_run.data_ingestion_analysis_needs_human`;
- `assistant_run.data_ingestion_analysis_failed`.

**Step 3: Add user-visible card shape**

For completed analysis, expose safe card fields:
- `row_count`;
- `warnings`;
- `mapping_plan_summary`;
- `staging_spec_summary`;
- `recommended_next_actions`.

Do not expose database URLs, credentials, or raw table dumps.

**Step 4: Connect to dataset/data-source staging**

Create a staged plan object in V3-owned storage or AssistantRun artifact state. It should be enough for a later operator-approved import to create/update a dataset, but must not mutate production data automatically.

**Step 5: Run tests**

Run:

```powershell
cargo test -p contracts codex_host_data_ingestion --lib
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api data_ingestion_analysis_output --lib
```

Expected: pass.

---

### Task 5: Make Answer-Quality Autofix Operational But Review-Gated

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`
- Test: `crates/codex-host-agent/src/lib.rs`
- Modify: `docs/validation/static-page-render-smoke.md`
- Modify: `scripts/run-v3-quality-gate-smoke.ps1`

**Step 1: Classify failures**

Normalize `answer_quality_autofix` failure types:
- `missing_source`;
- `parse_quality`;
- `retrieval_supply`;
- `answer_policy`;
- `not_reproducible`;
- `unsafe_or_out_of_scope`.

**Step 2: Add patch-scope tests**

Assert patches can only target allowlisted files and smoke fixtures:
- `crates/platform-api/src/lib.rs`;
- `fixtures/document-quality/**`;
- `scripts/run-document-quality-smoke.ps1`;
- `scripts/run-v3-quality-gate-smoke.ps1`;
- `docs/validation/**`.

**Step 3: Require test evidence**

Accept `patch_ready` only when output includes:
- changed files;
- tests added or updated;
- test commands;
- rollback notes;
- risk level.

**Step 4: Keep actual apply review-gated**

Do not auto-commit or auto-deploy. Produce an operator summary and candidate patch artifact.

**Step 5: Run tests**

Run:

```powershell
cargo test -p platform-api answer_quality_autofix --lib
cargo test -p codex-host-agent answer_quality --lib
```

Expected: pass.

---

### Task 6: Harden Executor Observability Without Constant Resource Use

**Files:**
- Modify: `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- Modify: `apps/web/app/api/v3/external/codex-executor-tasks/route.js`
- Modify: `apps/web/app/api/v3/external/codex-executor-tasks/[executionId]/runtime-inspect/route.js`
- Test: `apps/web/app/external-integrations/ExternalIntegrationsPageClient.test.mjs` if present, otherwise add focused route tests near existing external integration tests.

**Step 1: Preserve lazy loading**

Assert:
- task list is not fetched while panel is closed;
- runtime inspect is not fetched until one task is selected;
- closing the panel clears selected detail or stops further refresh.

**Step 2: Add filters**

Add lightweight filters:
- template id;
- status;
- latest first;
- limit.

Do not auto-refresh by default.

**Step 3: Improve detail display**

Show safe summaries first:
- template id;
- status;
- reason;
- retry count;
- assistant run id;
- artifact links;
- validation summary.

Keep raw JSON behind the existing preview area.

**Step 4: Run tests/build**

Run:

```powershell
npm test -- ExternalIntegrations
pnpm -C apps/web build
```

Expected: pass.

---

### Task 7: Define Retry, Timeout, Cancellation, And Workspace Retention

**Files:**
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/operations/codex-host-model-profiles.md`
- Test: `crates/codex-host-agent/src/main.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Document default policy**

Recommended defaults:
- short planning tasks: 2-5 minutes;
- static-page publish: 20-30 minutes;
- data-ingestion analysis: 10-30 minutes depending on sample size;
- max attempts: 3;
- no automatic retry for `needs_human`;
- retry only transient execution failures.

2026-05-27 progress: Cloudflare Codex static-page publish has been verified end-to-end on 8 server, and the Codex Host default task timeout has been raised to 30 minutes for demo safety. Poll exhaustion now records the remote Cloudflare task id, emits a processing retry event, and requeues the local task while attempt budget remains. The protected integrated observability page now loads workflow task attempts on demand and surfaces poll-retry/next-poll state. Remaining policy work: cancellation and retention hardening.

**Step 2: Add timeout/retry status summaries**

Expose safe statuses:
- `queued`;
- `running`;
- `retrying`;
- `timeout`;
- `failed`;
- `needs_human`;
- `completed`.

**Step 3: Verify cancellation**

Ensure `codex_host_execution_cancelled` is checked during long `codex_exec` runs and maps to a safe event.

**Step 4: Workspace retention**

Keep task workspaces for a bounded retention window for debugging. Cleanup must be explicit and backup-first when done locally in this workspace; server cleanup policy should be separate and documented.

**Step 5: Run tests**

Run:

```powershell
cargo test -p codex-host-agent cancellation --lib
cargo test -p platform-api codex_host_fixed_task --lib
```

Expected: pass.

---

### Task 8: Run Staged Smoke Before Real Production Enablement

**Files:**
- Modify: `docs/validation/static-page-render-smoke.md`
- Modify: `scripts/run-v3-quality-gate-smoke.ps1`
- Modify: `docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md`

**Step 1: Local regression**

Run:

```powershell
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api codex_host_fixed_task --lib
cargo test -p contracts codex_host --lib
npm run test:pure-third-party-guide-html
```

Expected: pass.

**Step 2: 8-server safe smoke**

With current 8-server `plan_only`/non-real-exec config, verify:
- static-page request returns `static_page_rendered`;
- response card has `render_output_id`;
- `codex_auto_publish_ready=false`;
- no Codex Host real task is launched.

**Step 3: Approved host real smoke**

Only after operator approval, use an approved `windows_jump`, `mac_host`, or `cloudflare_codex` host and verify:
- `codex_host_task.exec_completed`;
- fixed task output JSON extracted from stdout;
- static-page artifact URL under `/generated-artifacts/`;
- validation report present;
- AssistantRun gets final artifact event.

**Step 4: Data-ingestion smoke**

Use a non-sensitive sample database/table/document set and verify:
- `data_ingestion_analysis_queued`;
- `analysis_ready` or `staging_spec_ready`;
- no credential text in output;
- no production write;
- staging plan visible to operator/model.

**Step 5: Record results**

Update the main plan with:
- commit SHA;
- server deploy SHA;
- smoke case names;
- pass/fail summary;
- rollback steps.

---

## Recommended Development Order

1. Task 2: model-facing fixed-task status feed.
2. Task 3: static-page publish callback smoke and status mapping.
3. Task 6: observability filters and lazy-load assertions.
4. Task 4: data-ingestion result handoff.
5. Task 7: timeout/retry/cancellation/retention.
6. Task 5: answer-quality autofix review-gated loop.
7. Task 8: staged smoke and production readiness record.

## Stop Conditions

- Any request to change public third-party request fields.
- Any attempt to expose raw credentials, database URLs, full stdout/stderr, or provider payloads.
- Any executor output that wants to overwrite an existing generated artifact or stable URL.
- Any database schema or production data mutation without explicit human confirmation.
- Any host attempting `codex_exec` outside `windows_jump`, `mac_host`, or `cloudflare_codex`.
