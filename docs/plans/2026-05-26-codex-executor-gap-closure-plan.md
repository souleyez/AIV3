# Codex Executor Gap Closure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finish the Codex executor integration after the first queue/agent/observability bridge by closing business-result feedback, safety, retry, and smoke gaps.

**Architecture:** DataMax remains the system of record for task enqueue, permissions, datasets, artifacts, and third-party responses. Codex Host may execute fixed templates only after platform and agent readiness gates pass, and every result must return through DataMax validation, AssistantRun events, artifact manifests, or staged import plans.

**Tech Stack:** Rust `platform-api`, `contracts`, `workflow-definitions`, `codex-host-agent`; PostgreSQL workflow/AssistantRun events; Next.js external integration observability page; generated third-party integration docs; 8-server smoke scripts.

---

## Current State

- `CodexHostTask` workflow and `codex_host/run_codex_host_task` queue exist.
- `crates/codex-host-agent` supports `dry_run`, `plan_only`, gated local-host `codex_exec`, and `cloudflare_orchestrator` for the fixed Cloudflare Codex executor.
- Fixed templates exist for `static_page_image2_data_publish`, `answer_quality_autofix`, and `data_ingestion_analysis`.
- Static-page auto publish now accepts either old `codex_exec` readiness or the fixed `cloudflare_orchestrator + cloudflare_codex` readiness; otherwise DataMax returns direct HTML fallback with `render_output_id`.
- Integrated observability page has a protected Codex executor task panel; it only loads the task list when opened and only calls runtime inspect after selecting a task.
- Third-party AssistantRun reply polling now surfaces fixed-task states for queued/running/retrying/completed/needs-human/failed paths across static-page publish, data-ingestion analysis, and answer-quality diagnostics. Poll retry events are also included in safe Codex diagnostics summaries.

## Remaining Gaps

1. Static-page Codex publish has passed the core 8-server path, but still needs a named real smoke record for demo cases and rollback notes.
2. `data_ingestion_analysis` validates output, exposes safe task status, records completion/needs-human/failure events, attaches a safe staging plan artifact, can create/reuse a private staging dataset after operator confirmation, can start the existing ExternalSourceSync workflow for confirmed database-source plans, and records sync running/completed/failed status back to the AssistantRun; real environment smoke still needs coverage.
3. `answer_quality_autofix` can be represented and observed as a fixed task, now uses strict failure classification and review-gated patch-ready validation; remaining work is real smoke evidence and richer test-result feedback.
4. Operator-facing summaries are now usable, but model-side follow-up still needs broader wiring into ordinary assistant context for non-third-party surfaces.
5. Artifact manifests now cover static-page generated artifacts, Image2 preview visual contracts, data-ingestion analysis/staging plans, report render summary artifacts, and Codex executor runtime inspect.
6. Timeouts, retries, cancellation, concurrency budget, workspace retention, and cleanup policy need explicit operator rules and smoke coverage.
7. Real execution deployment is still guarded by host validation, credentials isolation, and task workspace behavior checks on the approved executor host.

---

### Task 1: Freeze The Executor Readiness Matrix

**Files:**
- Modify: `docs/operations/codex-host-model-profiles.md`
- Modify: `docs/operations/codex-jump-host-minimax-smoke.md`
- Modify: `docs/plans/2026-05-25-DataMax-mainline-quality-executor-plan.md`

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

2026-05-27 progress:
- Third-party status polling now translates fixed-task events into safe response cards for `data_ingestion_analysis`, `static_page_image2_data_publish`, and `answer_quality_autofix`.
- `codex_host_task.poll_retry` and heartbeat events are surfaced as retrying/running states with poll hints, without exposing raw logs or credentials.
- `workflow.step_failed` for a fixed Codex Host task now records a safe `codex_host.fixed_task.rejected` AssistantRun event.
- Safe diagnostics now count poll retries, heartbeats, and exec completions.
- Verified:
  - `cargo test -p platform-api fixed_task --lib`
  - `cargo test -p platform-api external_channel --lib`

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

Create a staged plan object in DataMax-owned storage or AssistantRun artifact state. It should be enough for a later operator-approved import to create/update a dataset, but must not mutate production data automatically.

**Step 5: Run tests**

Run:

```powershell
cargo test -p contracts codex_host_data_ingestion --lib
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api data_ingestion_analysis_output --lib
```

Expected: pass.

2026-05-27 progress:
- Accepted fixed-task outputs now append `assistant_run.data_ingestion_analysis_completed`, `assistant_run.data_ingestion_analysis_needs_human`, or `assistant_run.data_ingestion_analysis_failed`.
- Third-party polling now returns `reply.card.type=v3_data_ingestion_analysis_result` with a safe `result_summary` for completed/needs-human/failed data-ingestion analysis.
- AssistantRun output artifacts now carry `external_channel_data_ingestion_analysis` metadata for staging review, with explicit `production_write_allowed=false`, no credential exposure, and no raw table dump exposure.
- Completed data-ingestion outputs with mapping/staging content now also carry a reviewable `v3_data_ingestion_staging_plan` draft with stable `plan_id`, source-scope summary, target summary, mapping entries, staging steps, and fixed human-confirmation policy.
- Confirmed staging plans now create or reuse a private DataMax dataset and append `assistant_run.data_ingestion_staging_plan_confirmed`; third-party polling can surface `data_ingestion_staging_dataset_ready` without adding public request fields or importing raw rows automatically.
- DataMax internal operator route `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/confirm` now exposes that confirmation path behind the existing assistant-run visibility check.
- DataMax internal operator route `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/sync` now requires the confirmed plan, selects an in-plan database source, starts ExternalSourceSync into the confirmed staging dataset, records `assistant_run.data_ingestion_staging_sync_started`, and deduplicates repeat clicks by default.
- ExternalSourceSync workflow transitions that carry `connector_context.data_ingestion_staging` now append `assistant_run.data_ingestion_staging_sync_updated`, allowing third-party/model-visible status cards to move through running, completed, and failed states with stage/error context.
- Added `scripts/run-data-ingestion-staging-sync-smoke.sh` and `docs/validation/data-ingestion-staging-sync-smoke.md` as the safe local and 8-server checklist for the confirm/sync/worker materialization path.
- Verified:
  - `cargo test -p platform-api data_ingestion_analysis --lib`
  - `cargo test -p platform-api external_channel_data_ingestion --lib`
  - `cargo test -p platform-api data_ingestion --lib`
  - `cargo test -p platform-api external_source_sync --lib`
  - `cargo test -p external-source-worker`
  - `cargo test -p ingest-worker`
  - `cargo test -p retrieval-worker`

---

### Task 5: Make Answer-Quality Autofix Operational But Review-Gated

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`
- Test: `crates/codex-host-agent/src/lib.rs`
- Modify: `docs/validation/static-page-render-smoke.md`
- Modify: `scripts/run-DataMax-quality-gate-smoke.ps1`

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
- `scripts/run-DataMax-quality-gate-smoke.ps1`;
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

2026-05-27 progress: Cloudflare Codex static-page publish has been verified end-to-end on 8 server, and the Codex Host default task timeout has been raised to 30 minutes for demo safety. Poll exhaustion now records the remote Cloudflare task id, emits a processing retry event, and requeues the local task while attempt budget remains. The protected integrated observability page now loads workflow task attempts on demand and surfaces poll-retry/next-poll state. Cancellation now emits `codex_host_task.cancelled` with `retryable=false` and no raw prompt/stdout/stderr, and third-party polling can surface `*_cancelled` statuses. Fixed-task workspaces now write `runtime.json.retention_policy` with `CODEX_HOST_AGENT_TASK_WORKSPACE_RETENTION_HOURS` defaulting to 168 hours, `cleanup_requires_operator=true`, and `backup_before_delete=true`. Added `scripts/run-codex-host-workspace-retention-smoke.sh` for read-only cleanup-candidate reporting. Remaining policy work: real-environment retention scan evidence and data-ingestion staging smoke evidence.

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
- Modify: `scripts/run-cloudflare-codex-fixed-task-smoke.ps1`
- Modify: `scripts/run-DataMax-quality-gate-smoke.ps1`
- Modify: `docs/plans/2026-05-25-DataMax-mainline-quality-executor-plan.md`

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

2026-05-27 local pre-release smoke:
- `scripts/run-external-direct-reply-smoke.ps1` passed: ordinary external chat returns provider-authored content, fallback/rejection paths stay model-authored, and temporary document scopes keep the direct-reply contract.
- `scripts/run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case data-ingestion-analysis -Json` passed: contracts, codex-host-agent, platform-api data-ingestion, and fixed-task tests all passed; JSON report written under `target/cloudflare-codex-fixed-task-smoke/`.
- `scripts/run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case static-page-no-confirm -Json` passed: static-page no-confirm, fixed-task, and codex-host-agent static-page tests all passed; JSON report written under `target/cloudflare-codex-fixed-task-smoke/`.
- Fixed a Windows PowerShell smoke false failure where successful Cargo stderr lines were treated as terminating errors; the script now captures native command output with local `ErrorActionPreference=Continue` and still uses the real exit code.

2026-05-27 static-page status follow-up:
- Fixed a third-party polling edge where an older `assistant_run.external_channel_static_page_publish_queued` event could mask later `codex_host.fixed_task.*`, `codex_host_task.poll_retry`, or `codex_host_task.cancelled` events. Static-page replies now surface `static_page_publish_retrying`, `static_page_publish_failed`, `static_page_publish_needs_human`, and `static_page_publish_cancelled` instead of continuing to report publish queued/running.
- Static-page fixed-task success now uses the public contract status `static_page_published` even when the reply is built directly from fixed-task completion before the richer publish-completed event is appended.
- Focused checks passed: `cargo test -p platform-api external_channel_static_page_reply --lib`, `cargo test -p platform-api external_channel_fixed_task_reply_surfaces_completed_artifact_link --lib`, `cargo test -p platform-api external_channel_static_page --lib`, `npm run build:pure-third-party-guide-html`, `npm run test:pure-third-party-guide-html`, and `npm run check:pure-third-party-guide-html`.

2026-05-27 data-ingestion follow-up:
- External-channel follow-up turns now restore the latest `data_ingestion_staging` dataset for the same `conversation_external_id` when the third party does not resend a document/dataset range, but only if the latest sync update is `workflow_status=succeeded`; a later failed sync will not silently reuse stale staging data. This keeps the post-sync path natural: sync once, then continue Q&A/report/static-page requests over the staging dataset in the same conversation.
- The restored scope is explicit in `selected_scope.data_ingestion_staging_scope`, sets `datasets` and `canonical_datasets`, and does not cross conversation IDs.
- Focused checks passed: `cargo test -p platform-api external_channel_restores_completed_data_ingestion_staging_dataset_for_same_conversation --lib`, `cargo test -p platform-api external_channel --lib`, and `cargo test -p platform-api data_ingestion_staging --lib`.

2026-05-27 artifact manifest follow-up:
- Static-page generated artifacts and data-ingestion analysis/staging-plan artifacts now both include a safe `artifact_manifest` in AssistantRun `output_artifacts`, using `schema=DataMax.output_artifact_manifest` and `schema_version=1`.
- Report render summary HTML artifacts now expose the same manifest schema through `payload.artifactManifest`, so report outputs can be observed with the same `artifact_type` / `artifact_kind` / refs / safety shape.
- Static-page manifests expose the final generated-artifact URL through `primary_url` plus `public`/`download` links, and record safety flags such as `generated_artifact_only=true` and `overwrite_allowed=false`.
- Data-ingestion manifests expose the staging-plan identity and workflow execution refs without credentials, raw logs, or raw table dumps, and keep `production_write_allowed=false`.
- Focused checks passed: `cargo test -p platform-api external_channel_static_page_fixed_task_completion_appends_final_publish_event --lib`, `cargo test -p platform-api external_channel_data_ingestion_fixed_task_completion_appends_result_event --lib`, `cargo test -p platform-api html_artifact_report_render_summary_from_output_exposes_publishability --lib`, `cargo test -p platform-api external_channel_static_page --lib`, and `cargo test -p platform-api external_channel_data_ingestion --lib`.

2026-05-27 observability follow-up:
- `WorkflowRuntimeInspectView` now exposes a default-empty `artifact_manifests` list derived from the linked AssistantRun output artifacts. Runtime inspect only returns normalized `DataMax.output_artifact_manifest` objects and leaves raw artifacts, logs, prompts, credentials, and stdout/stderr out of the detail payload.
- The protected external integrations page renders the manifest list only inside a selected Codex executor task detail, preserving the existing lazy behavior: closed panel does not load tasks, and selecting one task is required before runtime inspect/manifest loading.
- Focused checks passed: `cargo test -p contracts workflow_runtime_inspect_view_defaults_pretty_summaries_when_missing --lib`, `cargo test -p platform-api output_artifact_manifests_from_output_artifacts_returns_only_safe_manifests --lib`, `cargo test -p platform-api external_channel --lib`, `node --test app/lib/external-integrations.test.mjs`, and `npm run build`.

2026-05-27 Image2 preview manifest follow-up:
- `static_page_image_job.preview_ready` events now include a safe `artifact_manifest` with `artifact_type=static_page_image_preview` and `artifact_kind=image2_visual_contract`.
- Runtime inspect now merges safe manifests from both AssistantRun `output_artifacts` and AssistantRun event payloads, with dedupe and strict manifest-field whitelisting. This allows preview-only visual-contract tasks to show up in the same centralized observability panel as final generated HTML artifacts.
- HTTPS preview assets are exposed as `primary_url` / `preview` links. Embedded data/blob preview assets are redacted from the manifest refs to avoid pushing large inline image data through observability.
- Focused checks passed: `cargo test -p static-page-worker static_page_image_preview_manifest`, `cargo test -p platform-api output_artifact_manifests_from_output_artifacts_returns_only_safe_manifests --lib`, and `cargo test -p platform-api external_channel_static_page_image_success_with_preview_enqueues_codex_once --lib`.

2026-05-27 answer-quality autofix follow-up:
- `answer_quality_autofix` output validation now requires normalized `failure_type` values from the agreed set, including `unsafe_or_out_of_scope`.
- `patch_ready` now requires changed files, tests added/updated, test commands, rollback notes, and an explicit valid risk level. Even low/medium risk patches remain `auto_apply_allowed=false` with `patch_review_required=true`, preserving the review-gated boundary.
- Fixed-task output summaries now expose whether rollback notes are present without exposing raw diff/provider logs.
- `codex-host-agent` fixed-task schema hint now advertises `unsafe_or_out_of_scope`.
- Focused checks passed: `cargo test -p platform-api answer_quality_autofix --lib` and `cargo test -p codex-host-agent answer_quality --lib`.

---

## Recommended Development Order

1. Real customer-style database dataset question/report smoke from a confirmed staging dataset, including row materialization, indexing, and generated report evidence.
2. Keep collecting 8-server smoke evidence for static-page publish failure/retry/cancelled cases and answer-quality autofix diagnostics.
3. Extend manifest coverage to any future generated artifact families as they are introduced.

## Stop Conditions

- Any request to change public third-party request fields.
- Any attempt to expose raw credentials, database URLs, full stdout/stderr, or provider payloads.
- Any executor output that wants to overwrite an existing generated artifact or stable URL.
- Any database schema or production data mutation without explicit human confirmation.
- Any host attempting `codex_exec` outside `windows_jump`, `mac_host`, or `cloudflare_codex`.
