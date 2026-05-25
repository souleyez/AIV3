# V3 Codex Executor Boundary Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Define and implement the product/technical boundary between V3 and the Codex executor so V3 owns state, permissions, datasets, and user experience, while Codex executes slow or complex generation tasks.

**Architecture:** V3 remains the system of record: datasets, documents, external IDs, permissions, task state, artifacts, and model-facing context are persisted in V3. Codex executor receives immutable task snapshots and returns artifact manifests, structured observations, and safe status messages. No executor task mutates V3 state directly; all writes go back through V3-controlled APIs or storage adapters.

**Tech Stack:** Rust platform API, contracts crate, codex-host-agent executor bridge, Postgres persistence, web static pages, third-party integration docs, static page renderer, Cloudflare GPT-image-2 queue.

---

## Boundary Decisions

### V3 Owns

- Dataset, document, database source, template, skill, and external ID mappings.
- Authentication, authorization, third-party channel config, and database credentials.
- Parse status, reparse status, task status, artifact versioning, and user-visible progress.
- Model-facing context summaries: what data exists, what is ready, what failed, and what the model may use.
- UI/observability: integration observation page, data source status, artifact zone, and external smoke results.

### Codex Executor Owns

- Slow or complex execution tasks: static page planning, HTML generation, GPT-image-2 visual drafts, report generation, template interpretation, database profiling, and quality smoke checks.
- Producing structured outputs: `artifact_manifest`, `quality_report`, `executor_observations`, `suggested_next_actions`.
- Returning deterministic status and failure categories, not ad hoc conversational fallbacks.

### Shared Contract

- Input must be an immutable snapshot: task ID, dataset IDs, document IDs, source versions, selected skills, output format, and user objective.
- Output must be a manifest: artifact locations, checksums or fingerprints, preview URLs, generated metadata, warnings, and failure details.
- Executor may suggest mutations but must not silently change datasets, document ownership, third-party mappings, or credentials.

---

## Task 1: Executor Task Envelope Contract

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: contract/unit tests in `crates/platform-api/src/lib.rs`

**Steps:**
1. Add or formalize an executor task envelope with fields:
   - `task_id`
   - `task_kind`
   - `requested_by`
   - `dataset_scope`
   - `document_scope`
   - `database_scope`
   - `requested_skills`
   - `output_format`
   - `input_snapshot`
   - `callback_policy`
2. Add tests that verify V3 can serialize the envelope without secrets.
3. Add tests that reject envelopes containing database passwords, raw tokens, or unscoped document access.
4. Run focused Rust tests for contracts/platform serialization.
5. Commit as executor envelope contract.

## Task 2: Artifact Manifest Contract

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: artifact manifest tests in platform API

**Steps:**
1. Define manifest fields:
   - `artifact_id`
   - `artifact_type`
   - `title`
   - `source_task_id`
   - `dataset_ids`
   - `document_ids`
   - `asset_keys`
   - `preview_asset_key`
   - `download_url`
   - `fingerprint`
   - `quality_report`
   - `warnings`
2. Ensure static pages, GPT-image-2 previews, report HTML, and JSON reports can share this shape.
3. Add tests for image artifact, static page artifact, and report artifact.
4. Run focused artifact manifest tests.
5. Commit as unified executor artifact manifest.

## Task 3: V3 Task State Ownership

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify if needed: storage/repository code currently near task, dataset output, and static page image job flows
- Test: status transition tests

**Steps:**
1. Define allowed status transitions:
   - `queued -> running -> completed`
   - `queued/running -> failed`
   - `failed -> retry_queued`
   - `running -> cancelled`
2. Add failure categories:
   - `executor_unavailable`
   - `timeout`
   - `model_failed`
   - `artifact_missing`
   - `input_snapshot_invalid`
   - `permission_scope_invalid`
3. Make model-facing context expose these statuses in plain language.
4. Add tests that failed or retrying tasks can be used by the model as context.
5. Commit as V3-owned executor task state.

## Task 4: Static Page and GPT-image-2 Handoff

**Files:**
- Modify: `crates/static-page-worker/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify if needed: `apps/web/app/lib/html-template-references.js`
- Test: static page image job tests and smoke docs

**Steps:**
1. Ensure V3 submits GPT-image-2 tasks without stale `projectId=codex-web`.
2. Persist the returned image artifact into the static page draft/image job manifest.
3. Ensure final HTML remains generated from structured request/data snapshot, not from the bitmap.
4. Add tests for image job success, image job failure, and retry.
5. Run a live smoke with a non-sensitive static page request.
6. Commit as static page executor handoff.

## Task 5: Database Understanding Executor Tasks

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: database source status/sync modules if separate
- Test: database source task envelope tests
- Docs: update database source integration notes

**Steps:**
1. Create executor task kinds:
   - `database_profile`
   - `database_metric_catalog`
   - `database_report_plan`
2. Snapshot only safe schema/sample summaries, never credentials.
3. Return table purpose, field semantics, metric candidates, time fields, dimensions, and report suggestions.
4. Store executor observations back onto the V3 data source/dataset readiness view.
5. Add tests for credential redaction and dataset binding.
6. Commit as database understanding executor handoff.

## Task 6: Template Skill Executor Tasks

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Docs: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Docs: `apps/web/public/external-integrations/pure-third-party-integration-guide.zh-CN.md`

**Steps:**
1. Define task kinds:
   - `template_document_profile`
   - `template_report_generate`
2. Preserve current third-party interface semantics; only use existing `requested_skills` payload.
3. Store template profile as V3-owned metadata.
4. Return generated report/static page as an artifact manifest.
5. Add tests for required template skill, preferred template skill, and missing template document.
6. Commit as template skill executor handoff.

## Task 7: Observation Page Integration

**Files:**
- Modify: `apps/web/app` observation page components
- Modify: platform API observation endpoints if needed
- Test: browser smoke for observation page

**Steps:**
1. Add executor task list to the centralized observation page.
2. Load details only when a task is selected.
3. Do not poll all conversations or tasks permanently.
4. Protect external conversation test panels behind the existing key/access gate.
5. Add smoke that verifies idle page does not start heavy polling.
6. Commit as lazy executor observation.

## Task 8: End-to-End Smoke Matrix

**Files:**
- Create or update: `docs/validation/executor-boundary-smoke.md`
- Update related smoke scripts only if necessary

**Steps:**
1. Smoke static page HTML generation.
2. Smoke GPT-image-2 visual preview.
3. Smoke template skill report generation.
4. Smoke database profile task with redacted credentials.
5. Smoke third-party document move and scoped chat after move.
6. Smoke failed executor task status feeding back to model context.
7. Record expected outputs and residual risks.
8. Commit validation results.

---

## Execution Order

1. Contract first: task envelope and artifact manifest.
2. State ownership: status transitions and model-facing failure context.
3. Static page/GPT-image-2 handoff because it is already manually proven.
4. Database understanding handoff because it unlocks enterprise reporting quality.
5. Template skill handoff.
6. Observation page lazy loading.
7. Full smoke matrix.

## Non-Goals

- Do not let Codex executor directly mutate V3 datasets or document ownership.
- Do not expose database credentials, tokens, or raw third-party secrets in task payloads.
- Do not change public third-party request semantics without explicit confirmation.
- Do not make observation pages poll all task details by default.
- Do not treat GPT-image-2 bitmap output as the final HTML source of truth.

## Acceptance Criteria

- V3 can submit executor tasks with scoped immutable snapshots.
- Executor can return manifests for static pages, images, reports, and database profiles.
- V3 persists all state and exposes status to UI and model context.
- Third-party integrations can keep using existing document, dataset, skill, and output format fields.
- Failed, retrying, and parsing states are visible to the model in a way that supports useful answers.
- Observation page remains lazy and protected for conversation tests.
