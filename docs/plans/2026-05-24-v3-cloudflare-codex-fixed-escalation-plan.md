# V3 Cloudflare Codex Fixed Escalation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let V3 safely delegate fixed advanced workflows to Cloudflare Codex without per-task human confirmation: Image2-first static-page publishing, low-quality answer auto-optimization, and customer-requested data ingestion / database analysis.

**Architecture:** V3 remains the authority for user scope, dataset visibility, queue submission, artifact publication, audit, and rollback. Cloudflare Codex is treated as an execution host that already has operational access to the 8 server and V3, but it can only run server-defined task templates with fixed inputs, fixed output schemas, allowlisted capabilities, and write scopes. Human confirmation moves from every task to template approval and exception handling.

**Tech Stack:** Rust `platform-api`, `contracts`, `codex-host-agent`, `storage`, `workflow-definitions`; Next.js static-page draft helpers; existing AssistantRun ReAct and answer-quality code; Cloudflare Codex/Image2 queue; generated artifacts on 8 server; PostgreSQL audit/runtime events; focused local and private 8-server smokes.

---

## Non-Negotiable Constraints

- Do not change third-party public URLs, auth, request fields, response fields, or documented API behavior.
- Do not re-enable the rolled-back customer-facing answer quality gate as a blocking runtime gate.
- Do not let user text directly select Codex CLI flags, model profiles, workspace roots, database credentials, or deployment commands.
- Do not put provider keys, V3 secrets, raw database URLs, browser tokens, or server SSH details into Codex task prompts.
- Do not let Cloudflare Codex become a browser-facing API, dataset permission authority, or memory authority.
- Do not overwrite existing customer artifacts automatically. The no-confirm static-page path may create and publish a new generated artifact only.
- Do not allow low-quality answer auto-optimization to modify unrelated product code, public integrations, auth, billing, deploy scripts, database schema, or static-page UI.
- Do not let data-ingestion analysis expose raw database credentials, create public API fields, modify production schema, or import customer data into a broader scope than V3 selected.
- Keep every automatic loop bounded, auditable, replayable, and reversible.

## Operating Assumption

Cloudflare Codex is assumed to already have the required V3 and 8-server operations permissions. V3 should still validate this as an execution-environment readiness check, not as a reason to broaden task prompts.

Readiness means:

- Codex Host diagnostics show an approved remote host kind, not the local developer workstation.
- The host can reach V3 internal services needed for its fixed template.
- The host can write only to the allowed generated-artifacts area or isolated task workspace.
- V3 receives a structured output that can be validated before any customer-visible action is recorded.

## Target Decision

Use template-level approval:

```text
operator approves template once
  -> V3 stores template policy and allowlist
  -> routine task instances run without human confirmation
  -> V3 validates output and records audit
  -> exception cases request human confirmation by email/runtime alert
```

Approved fixed templates:

1. `static_page_image2_data_publish`
2. `answer_quality_autofix`
3. `data_ingestion_analysis`

## Current Status: 2026-05-25

Implemented or mostly implemented:

- Fixed template policy docs, contract views, host preflight, and audit event summaries exist for `static_page_image2_data_publish` and `answer_quality_autofix`.
- `static_page_image2_data_publish` can be queued from V3 as a fixed Codex Host task when `CODEX_HOST_TASK_ENABLED=true` and the capability is in `CODEX_HOST_TASK_ALLOWLIST`.
- Third-party complex static-page requests can be detected without adding third-party request fields by using existing `render_mode: "artifact"`, `output_format: "image_text"`, and optional requested skill `output_type: "static_page"`.
- The third-party static-page path now queues Image2 first, emits a stream/status card for the effect image, and does not wait for customer confirmation before continuing.
- After the Image2 workflow succeeds and a preview asset exists, V3 can enqueue the fixed `static_page_image2_data_publish` Codex Host task automatically.
- `codex-host-agent` fixed-template real execution now requires and parses structured fixed JSON output, storing it under `fixed_task_output` for V3 validation/audit.
- Fixed Codex Host runs now materialize a standard workspace bundle with `task.json`, `schemas/output.schema.json`, `evidence/summary.json`, `runtime.json`, and `README.md`.
- Static-page fixed-task preflight now rejects missing Image2 preview assets, missing selected data/document/database scope, missing snapshot/trend/unit/detail policies, or attempts to require customer confirmation before continuing.
- Successful `static_page_image2_data_publish` fixed-task output now appends `assistant_run.external_channel_static_page_publish_completed`, records the final generated-artifact URL, keeps a validation summary, and can be rendered through the existing third-party `artifact_link` reply shape.
- `codex-host-agent` real `codex_exec` now uses bounded async process execution with timeout, heartbeat events, cancellation checks, configurable stdout/stderr caps, and safe non-zero/timeout failure reasons.
- The local fixed-task smoke now includes `static-page-no-confirm`, covering the Image2 no-confirm pending card, preview-ready queue gate, fixed output validation, and final `artifact_link` reply conversion. A read-only 8-server `-PlanOnly` readiness check passed on 2026-05-25; mutation smoke remains operator-gated.
- `data_ingestion_analysis` is now represented as a first-class fixed template contract with examples, host preflight, output validation, redaction/sensitive-text checks, and model-profile documentation for read-only analysis or staging-spec proposals.
- Third-party/customer messages can now trigger `data_ingestion_analysis` from existing chat text and V3-selected scope only. Static-page artifact requests still take precedence; data-ingestion replies use existing `task_status` and `v3_data_ingestion_analysis` card fields.
- The fixed-task smoke script now includes `data-ingestion-analysis`; local plan-only smoke passed and 8-server plan-only readiness passed with remote mutation guarded.
- `answer_quality_autofix` has bounded case packaging, allowlisted write scope, output validation, and audit summaries, but should remain patch-proposal oriented until the execution loop is proven.
- `data_ingestion_analysis` is newly approved as a fixed-template direction: a customer request is enough to trigger packaging, but only through existing message fields, V3-selected source scope, and no-confirm read-only/proposal actions.

Remaining executor gaps:

- Data-ingestion tasks still need explicit operator-approved private mutation smoke before `codex_exec` is enabled for real analysis/spec generation.
- Private 8-server mutation smoke must still prove the full chain after explicit operator approval: third-party request -> Image2 preview -> Codex Host execution -> fixed JSON output -> V3 validation -> generated-artifact URL -> final third-party-visible status.

## Template 1: `static_page_image2_data_publish`

Purpose:

- Convert an advanced static-page request into an Image2 visual contract.
- Bind real V3 data after reading the image/design direction.
- Validate snapshot/date/unit口径.
- Publish a new generated artifact on the 8 server.

May run without per-task human confirmation when all are true:

- It creates a new artifact path under `/generated-artifacts/`.
- It uses V3-selected dataset/source scope only.
- It includes a validation report with snapshot policy, unit policy, row counts, and source summaries.
- It does not overwrite or revoke any previous artifact.
- It does not change app source code, public API, auth, database schema, or deployment config.

Must require human confirmation when:

- It wants to overwrite an existing artifact or publish over a stable customer URL.
- It asks for new data permissions or credentials.
- It changes V3 source code or runtime configuration.
- It cannot prove data口径, latest snapshot, or unit conversion.
- It needs to send the result to a customer channel outside V3's normal artifact link return.

Fixed input package:

```json
{
  "template_id": "static_page_image2_data_publish",
  "version": 1,
  "draft_id": "uuid",
  "assistant_run_id": "uuid",
  "dataset_scope": {
    "tenant_id": "uuid",
    "dataset_ids": ["uuid"],
    "database_source_ids": ["uuid"],
    "selected_document_ids": ["uuid"]
  },
  "requirements": {
    "user_goal": "string",
    "project_name": "string",
    "time_dimension_required": true,
    "primary_partition_required": true,
    "detail_table_required": true
  },
  "image2": {
    "prompt_text": "string",
    "image_job_id": "string",
    "visual_contract_url": "https://..."
  },
  "policies": {
    "snapshot_aggregation": "latest_snapshot_for_state_modules",
    "trend_aggregation": "date_series_only_for_trends",
    "unit_rendering": "validate_raw_value_then_choose_wan_or_yi",
    "publish_mode": "new_generated_artifact_only"
  }
}
```

Fixed output schema:

```json
{
  "template_id": "static_page_image2_data_publish",
  "status": "success|needs_human|failed",
  "artifact": {
    "local_path": "string",
    "public_url": "https://v3.elepcloud.com/generated-artifacts/...",
    "manifest_path": "string"
  },
  "validation_report": {
    "snapshot_policy": "string",
    "latest_snapshot": "string|null",
    "source_row_count": 0,
    "current_state_row_count": 0,
    "detail_row_count": 0,
    "unit_policy": "string",
    "warnings": ["string"]
  },
  "source_summary": ["string"],
  "human_review_reason": "string|null"
}
```

## Template 2: `answer_quality_autofix`

Purpose:

- Collect low-quality answer cases asynchronously.
- Decide whether the issue is missing source data, poor parsing, weak retrieval/supply, or answer-generation logic.
- If it is a system answer-quality defect, generate a narrowly scoped optimization patch plus regression tests.

May run without per-task human confirmation when all are true:

- It works from V3-collected low-quality cases, not arbitrary user instructions.
- It touches only answer-quality prompt/policy/eval/smoke surfaces.
- It adds or updates regression cases before changing behavior.
- Local tests and selected smoke cases pass.
- It produces an audit entry with failure type, changed files, tests, and rollback pointer.

Must require human confirmation when:

- It wants to change public APIs, auth, request/response fields, database schema, third-party integration docs, deploy scripts, billing, or static-page product code.
- It wants to broaden dataset permissions or expose raw customer data outside the fixed task package.
- It cannot reproduce the failure locally or with a private smoke.
- It proposes changes outside the answer-quality allowlist.
- It wants to deploy directly after patching.

Fixed input package:

```json
{
  "template_id": "answer_quality_autofix",
  "version": 1,
  "case_id": "uuid",
  "assistant_run_id": "uuid",
  "low_quality_signals": ["user_complaint", "weak_insufficient_evidence_answer"],
  "user_question": "string",
  "customer_answer": "string",
  "evidence_summary": {
    "selected_scope": {},
    "supply_quality": {},
    "answer_supply_sources": ["string"],
    "retrieval_or_fact_snapshot_status": "string"
  },
  "trace_summary": {
    "react_actions": ["string"],
    "quality_gate_events": ["string"],
    "parse_quality": ["string"]
  },
  "allowed_write_scope": {
    "files": [
      "crates/platform-api/src/lib.rs",
      "fixtures/document-quality/**",
      "scripts/run-document-quality-smoke.ps1",
      "scripts/run-v3-quality-gate-smoke.ps1",
      "docs/validation/**"
    ],
    "symbols": [
      "assistant_run_answer_quality_*",
      "assistant_run_react_*",
      "assistant_run_supply_quality_*"
    ]
  }
}
```

Fixed output schema:

```json
{
  "template_id": "answer_quality_autofix",
  "status": "patch_ready|needs_human|not_system_defect|failed",
  "failure_type": "missing_source|parse_quality|retrieval_supply|answer_policy|not_reproducible",
  "root_cause": "string",
  "changed_files": ["string"],
  "tests_added": ["string"],
  "test_commands": ["string"],
  "risk_level": "low|medium|high",
  "rollback_notes": "string",
  "human_review_reason": "string|null"
}
```

## Template 3: `data_ingestion_analysis`

Purpose:

- Let customers ask for data接入, 入库, 建表, 字段映射, 数据质量检查, schema 盘点, or database/source analysis in natural language.
- Package the request into a bounded Codex Host task that profiles only V3-selected sources and available files.
- Produce an ingestion/specification proposal, mapping plan, validation SQL, data-quality report, and recommended next actions.
- Optionally prepare V3-managed staging/import job specs when the source is already configured and the target scope is new/staging-only.

May run without per-task human confirmation when all are true:

- The trigger is a customer request in an existing V3/third-party message; no new public request field is required.
- The task uses only V3-selected documents, uploaded files, database source ids, or already configured external source previews.
- The first pass is read-only analysis, schema/sample profiling, mapping proposal, or staging-job specification.
- Any write target is new, isolated, V3-managed staging/proposed state, not a production/stable table or public API.
- The output includes source summary, mapping confidence, validation queries/checks, row/sample counts, and uncertainty warnings.

Must require human confirmation when:

- New credentials, connection strings, SSH access, browser tokens, or data-source permission expansion is required.
- The task wants to alter production schema, overwrite existing tables, delete data, change ingestion schedule, or publish a new public integration contract.
- The source scope is ambiguous, cross-tenant, or not visible in V3-selected context.
- The host cannot explain field mapping, primary keys, date columns, unit conventions, dedupe policy, or quality risks.
- The task asks to deploy ingestion code, run migration scripts, or change auth/public API behavior.

Fixed input package:

```json
{
  "template_id": "data_ingestion_analysis",
  "version": 1,
  "assistant_run_id": "uuid",
  "request": {
    "user_goal": "string",
    "intent": "connect_source|ingest_file|profile_database|build_table|data_quality|schema_mapping",
    "customer_priority": "normal|high"
  },
  "dataset_scope": {
    "tenant_id": "uuid",
    "dataset_ids": ["uuid"],
    "database_source_ids": ["uuid"],
    "selected_document_ids": ["uuid"],
    "uploaded_file_refs": ["string"]
  },
  "source_context": {
    "known_sources": ["string"],
    "schema_summaries": [],
    "sample_row_summaries": [],
    "document_summaries": []
  },
  "policies": {
    "mode": "read_only_analysis_or_staging_spec",
    "credential_policy": "do_not_request_or_emit_credentials",
    "production_write_policy": "needs_human_confirmation",
    "public_api_change_allowed": false,
    "schema_change_allowed_without_confirmation": false
  }
}
```

Fixed output schema:

```json
{
  "template_id": "data_ingestion_analysis",
  "status": "analysis_ready|staging_spec_ready|needs_human|failed",
  "intent": "connect_source|ingest_file|profile_database|build_table|data_quality|schema_mapping",
  "source_summary": ["string"],
  "proposed_ingestion": {
    "target_kind": "staging_table|dataset_profile|mapping_spec|none",
    "target_name": "string|null",
    "primary_keys": ["string"],
    "date_columns": ["string"],
    "dedupe_policy": "string",
    "refresh_policy": "string"
  },
  "mapping_plan": [
    {
      "source_field": "string",
      "target_field": "string",
      "confidence": "high|medium|low",
      "notes": "string"
    }
  ],
  "data_quality_report": {
    "sample_row_count": 0,
    "null_risks": ["string"],
    "type_risks": ["string"],
    "duplicate_risks": ["string"],
    "date_or_unit_risks": ["string"]
  },
  "validation_checks": ["string"],
  "recommended_next_actions": ["string"],
  "human_review_reason": "string|null"
}
```

## Task 1: Add Template Policy Documentation

**Files:**

- Create: `docs/operations/cloudflare-codex-fixed-task-templates.md`
- Modify: `docs/operations/static-page-image2-data-publish.md`
- Modify: `docs/operations/codex-host-model-profiles.md`
- Modify: `docs/architecture/codex-host-bridge-contract.md`

**Step 1: Document the two fixed templates**

Write `docs/operations/cloudflare-codex-fixed-task-templates.md` with:

- template ids;
- input packages;
- output schemas;
- no-confirm conditions;
- human-confirm exception conditions;
- audit events;
- rollback rules.

**Step 2: Update static-page operations doc**

In `docs/operations/static-page-image2-data-publish.md`, replace the current "human confirmation before publishing" later-phase language with the new narrower rule:

- new generated artifact can publish without per-task confirmation;
- overwrite, stable URL replacement, source-code change, or uncertain口径 still requires confirmation.

**Step 3: Update model profiles doc**

In `docs/operations/codex-host-model-profiles.md`, add:

```toml
[profiles.cloudflare-codex-fixed-tasks]
kind = "codex-native"
enabled = false
transport = "exec_schema"
model = "gpt-5.3-codex"
allowed_capabilities = ["static_page_image2_data_publish", "answer_quality_autofix", "data_ingestion_analysis"]
```

Also document that this profile may only run server-owned templates, not arbitrary user prompts.

**Step 4: Update bridge contract**

In `docs/architecture/codex-host-bridge-contract.md`, add a "Fixed Task Template" section explaining:

- template id is mandatory;
- raw task text is optional and bounded;
- output is accepted only if it matches the template schema;
- direct host permissions do not bypass V3 validation.

**Step 5: Verify docs**

Run:

```powershell
rg -n "static_page_image2_data_publish|answer_quality_autofix|data_ingestion_analysis|cloudflare-codex-fixed-tasks" docs
```

Expected: all four docs mention the fixed-template policy consistently.

## Task 2: Add Contract Types For Fixed Templates

**Files:**

- Modify: `crates/contracts/src/lib.rs`

**Step 1: Add failing contract tests**

Add tests near existing `CodexHostTaskRequestView` tests:

```rust
#[test]
fn codex_host_static_page_template_context_round_trips() {
    let context = CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
    let encoded = serde_json::to_value(&context).expect("encoded");
    assert_eq!(encoded["template_id"], json!("static_page_image2_data_publish"));
    assert_eq!(encoded["policies"]["publish_mode"], json!("new_generated_artifact_only"));
}

#[test]
fn codex_host_answer_quality_template_context_round_trips() {
    let context = CodexHostFixedTaskTemplateContextView::answer_quality_autofix_example();
    let encoded = serde_json::to_value(&context).expect("encoded");
    assert_eq!(encoded["template_id"], json!("answer_quality_autofix"));
    assert!(encoded["allowed_write_scope"]["files"].as_array().unwrap().iter().any(|value| {
        value == "fixtures/document-quality/**"
    }));
}

#[test]
fn codex_host_data_ingestion_template_context_round_trips() {
    let context = CodexHostFixedTaskTemplateContextView::data_ingestion_analysis_example();
    let encoded = serde_json::to_value(&context).expect("encoded");
    assert_eq!(encoded["template_id"], json!("data_ingestion_analysis"));
    assert_eq!(encoded["policies"]["mode"], json!("read_only_analysis_or_staging_spec"));
}
```

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p contracts codex_host_ --lib
```

Expected: new tests fail because the fixed-template context type does not exist.

**Step 3: Add private-safe serializable views**

Add serde views:

- `CodexHostFixedTaskTemplateIdView`
- `CodexHostFixedTaskTemplateContextView`
- `CodexHostFixedTaskOutputView`
- `CodexHostFixedTaskHumanReviewPolicyView`
- `CodexHostFixedTaskWriteScopeView`

Keep them generic enough for both templates, but avoid a free-form "anything" contract. Use typed fields for template id, status, validation report, changed files, tests, and human-review reason.

**Step 4: Verify**

Run:

```powershell
cargo test -p contracts codex_host_ --lib
```

Expected: contract tests pass and existing Codex Host request/output tests remain green.

**Step 5: Commit**

```powershell
git add crates/contracts/src/lib.rs docs/operations docs/architecture
git commit -m "feat: define fixed Cloudflare Codex task templates"
```

## Task 3: Add Codex Host Agent Template Preflight

**Files:**

- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `crates/codex-host-agent/src/main.rs` only if event naming needs template ids.

**Step 1: Add failing preflight tests**

Add tests:

- `plan_only_allows_static_page_image2_data_publish_template`
- `plan_only_allows_answer_quality_autofix_template`
- `plan_only_allows_data_ingestion_analysis_template`
- `codex_exec_rejects_untemplated_write_capability`
- `static_page_template_requires_generated_artifact_publish_mode`
- `answer_quality_template_rejects_non_allowlisted_file_scope`
- `data_ingestion_template_rejects_production_schema_write_without_confirmation`

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p codex-host-agent --lib
```

Expected: tests fail until template-aware preflight exists.

**Step 3: Implement template-aware preflight**

Extend `CodexHostAgentPolicy::prepare` so write-capable or publish-capable capabilities require:

- known `template_id`;
- profile allowlist contains the template capability;
- template package validates no-confirm policy;
- `CODEX_HOST_AGENT_HOST_KIND` is approved remote host kind;
- `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT` exists for any patch-generating task;
- static-page publish mode is `new_generated_artifact_only`;
- answer-quality write scope matches the allowlist.

**Step 4: Add safe event fields**

Include only safe summaries in output:

- `template_id`
- `template_status`
- `human_review_required`
- `human_review_reason`
- `changed_file_count`
- `artifact_public_url` when present

Do not include raw prompts, raw diffs, credentials, command args, or database URLs.

**Step 5: Verify**

Run:

```powershell
cargo test -p codex-host-agent --lib
cargo test -p contracts codex_host_ --lib
```

Expected: template preflight passes and untemplated write/publish attempts are rejected.

**Step 6: Commit**

```powershell
git add crates/codex-host-agent/src/lib.rs crates/codex-host-agent/src/main.rs crates/contracts/src/lib.rs
git commit -m "feat: preflight fixed Codex Host templates"
```

## Task 4: Wire Static-Page Advanced Publish

**Files:**

- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/lib/static-page-draft.test.mjs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: existing static-page tests in `platform-api`

**Step 1: Add frontend payload tests**

Extend `apps/web/app/lib/static-page-draft.test.mjs` to assert advanced static-page payloads include:

```js
assert.equal(payload.productionRules.codexHostEscalation.templateId, 'static_page_image2_data_publish');
assert.equal(payload.productionRules.codexHostEscalation.confirmationPolicy, 'auto_for_new_generated_artifact');
```

**Step 2: Run frontend tests and verify failure**

Run:

```powershell
node --test apps/web/app/lib/static-page-draft.test.mjs
```

Expected: tests fail until the payload carries the fixed template policy.

**Step 3: Add static-page template id to production rules**

Update `buildStaticPageImageProductionRules` so store/brand/database-backed pages include:

- `templateId: "static_page_image2_data_publish"`
- `confirmationPolicy: "auto_for_new_generated_artifact"`
- `publishMode: "new_generated_artifact_only"`
- existing snapshot/trend/unit policies.

**Step 4: Add platform-api enqueue tests**

In `crates/platform-api/src/lib.rs`, add focused tests near static-page/Codex Host helper tests:

- advanced Image2 static page creates `codex_host_task` with `template_id=static_page_image2_data_publish`;
- static page overwrite request sets `requires_confirmation=true`;
- missing/uncertain口径 returns `needs_human` instead of publishing.

**Step 5: Implement enqueue path**

When static-page render intent matches advanced Image2 workflow and Codex Host is enabled/allowlisted:

- build the fixed static-page package;
- enqueue `codex_host_task_workflow`;
- keep normal direct static-page behavior as fallback when host execution is disabled;
- publish only after V3 validates returned output schema and generated artifact path.

**Step 6: Verify**

Run:

```powershell
node --test apps/web/app/lib/static-page-draft.test.mjs
cargo test -p platform-api static_page --lib
cargo test -p platform-api codex_host --lib
```

Expected: advanced static-page payload and backend enqueue behavior are pinned.

**Step 7: Commit**

```powershell
git add apps/web/app/lib/static-page-draft.js apps/web/app/lib/static-page-draft.test.mjs crates/platform-api/src/lib.rs
git commit -m "feat: queue fixed Codex static-page publishing tasks"
```

## Task 5: Add Low-Quality Answer Case Collector

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs` only if a new durable low-quality case table is needed.
- Create: `docs/operations/answer-quality-autofix.md`

**Step 1: Add failing collector tests**

Add tests near answer-quality tests:

- user complaint after an answer records a low-quality case;
- answer says "资料不足/不够回答" while evidence exists records a low-quality case;
- accepted spreadsheet/table answer does not record a low-quality case;
- no customer-facing blocking gate is enabled by the collector.

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p platform-api assistant_run_answer_quality --lib
```

Expected: new collector tests fail until low-quality case recording exists.

**Step 3: Implement passive low-quality signals**

Collect signals from existing events and responses:

- direct user dissatisfaction or strong complaint;
- weak insufficient-evidence language when evidence/facts exist;
- retry exhausted or controlled fallback events;
- parse-quality degraded but no upgrade attempted;
- no citations/source summary for structured table/statistics/ranking requests;
- private smoke failures.

Do not block or replace customer answers in this task.

**Step 4: Store bounded case package**

Store only:

- assistant run id;
- question;
- answer excerpt;
- selected scope summary;
- supply quality summary;
- source ids/counts;
- event names and compact trace;
- low-quality signal names.

Do not store raw secrets, provider payloads, or full stdout/stderr.

**Step 5: Add docs**

Document `answer_quality_autofix` in `docs/operations/answer-quality-autofix.md`:

- signal definitions;
- non-blocking behavior;
- fixed Codex package;
- auto-patch allowlist;
- rollback.

**Step 6: Verify**

Run:

```powershell
cargo test -p platform-api assistant_run_answer_quality --lib
rg -n "answer_quality_autofix|low-quality case|低质量" docs/operations
```

Expected: collector works without reactivating runtime blocking.

**Step 7: Commit**

```powershell
git add crates/platform-api/src/lib.rs crates/storage/src/lib.rs docs/operations/answer-quality-autofix.md
git commit -m "feat: collect low-quality answer cases for autofix"
```

## Task 6: Wire `answer_quality_autofix` Codex Tasks

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/workflow-definitions/src/lib.rs` if a dedicated workflow stage is needed.
- Modify: `fixtures/document-quality/smoke-cases.json`
- Modify: `scripts/run-document-quality-smoke.ps1`
- Modify: `scripts/run-v3-quality-gate-smoke.ps1`

**Step 1: Add failing enqueue tests**

Add tests:

- collected system-defect case enqueues fixed template `answer_quality_autofix`;
- missing-source case is marked `not_system_defect` and does not patch;
- proposed patch outside allowlist becomes `needs_human`;
- tests are required before a low-risk patch can be marked auto-applicable.

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p platform-api answer_quality_autofix --lib
```

Expected: tests fail before enqueue/output handling exists.

**Step 3: Implement fixed task package builder**

Build the `answer_quality_autofix` package from recorded case data, including:

- question;
- customer answer;
- evidence summary;
- compact trace;
- low-quality signals;
- allowed write scope;
- required tests/smokes.

**Step 4: Add output validation**

Accept host output only when:

- `template_id=answer_quality_autofix`;
- status is one of the fixed enum values;
- changed files are within allowlist;
- tests were added or existing specific regression cases were updated;
- test command list is present;
- risk level is not high for auto-apply.

**Step 5: Keep deploy manual**

Even when per-task human confirmation is skipped for diagnosis and patch generation, production deployment remains a separate release step unless the operator later approves a deploy-specific template.

**Step 6: Verify**

Run:

```powershell
cargo test -p platform-api answer_quality_autofix --lib
cargo test -p platform-api assistant_run_answer_quality --lib
.\scripts\run-v3-quality-gate-smoke.ps1 -Local -Case all
```

Expected: low-quality cases can produce fixed Codex tasks; normal answers are not blocked.

**Step 7: Commit**

```powershell
git add crates/platform-api/src/lib.rs crates/workflow-definitions/src/lib.rs fixtures/document-quality scripts
git commit -m "feat: queue fixed Codex answer-quality autofix tasks"
```

## Task 7: Add Audit, Runtime Inspect, And Email Exception Path

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/components/ChatPanel.js` only if existing runtime diagnostics need a visible status badge.
- Modify: Gmail/Cloudflare email integration docs only if an existing configured email path is already documented.

**Step 1: Add event tests**

Add tests that fixed tasks emit:

- `codex_host.fixed_task.queued`
- `codex_host.fixed_task.completed`
- `codex_host.fixed_task.needs_human`
- `codex_host.fixed_task.rejected`

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p platform-api codex_host_fixed_task --lib
```

Expected: tests fail before event mapping is implemented.

**Step 3: Implement audit events**

Events must include:

- template id;
- assistant run id;
- status;
- public artifact url or changed file count;
- test commands;
- human review reason.

Events must exclude:

- raw prompt;
- raw diff;
- secrets;
- database URLs;
- provider logs.

**Step 4: Add human exception routing**

Use the existing configured Cloudflare Codex/email path to notify `soulzyn@qq.com` only when:

- fixed output says `needs_human`;
- host preflight rejects a task that the user expected to auto-run;
- output validation fails;
- proposed answer-quality patch is medium/high risk.

Do not email on every successful routine fixed-template run.

**Step 5: Verify**

Run:

```powershell
cargo test -p platform-api codex_host_fixed_task --lib
cargo test -p platform-api assistant_run_detail --lib
```

Expected: runtime inspect can show fixed-task status without exposing sensitive internals.

**Step 6: Commit**

```powershell
git add crates/platform-api/src/lib.rs apps/web/app/components/ChatPanel.js docs/operations
git commit -m "feat: audit fixed Codex task execution"
```

## Task 8: Private 8-Server Smoke And Rollout

**Files:**

- Modify: `docs/validation/static-page-render-smoke.md`
- Modify: `docs/validation/document-understanding-smoke.md`
- Modify: `docs/validation/assistant-chat-contract-smoke.md`
- Create or update: `scripts/run-cloudflare-codex-fixed-task-smoke.ps1`

**Step 1: Add smoke script**

Create `scripts/run-cloudflare-codex-fixed-task-smoke.ps1` with cases:

- static page new artifact dry-run/plan-only;
- static page new artifact exec on approved host;
- static page overwrite rejected or requires human;
- low-quality answer case produces `answer_quality_autofix`;
- out-of-scope patch is rejected.

**Step 2: Verify local no-network path**

Run:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly
```

Expected: the script validates package shape and rejection paths without touching 8 server.

**Step 3: Deploy only after review**

When operator approves deployment:

```powershell
git status --short
git log -5 --oneline
```

Then use the existing V3 deploy path for 8 server. Do not touch 120 server.

**Step 4: Run private 8-server smoke**

Run:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -Case all
```

Expected:

- static-page new artifact returns public generated-artifact URL;
- no overwrite happens;
- answer-quality autofix creates a patch proposal or marks not-system-defect;
- runtime inspect shows fixed-task audit events.

**Step 5: Rollout gates**

Enable in this order:

1. `plan_only` for both templates.
2. static-page `codex_exec` for new generated artifacts only.
3. answer-quality diagnosis and patch proposal.
4. answer-quality low-risk auto-apply in isolated workspace.
5. deployment automation only if a later separate deploy template is approved.

**Step 6: Rollback**

Rollback by disabling:

```text
CODEX_HOST_TASK_ENABLED=false
CODEX_HOST_TASK_ALLOWLIST=
CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES=
```

Also keep static-page direct fallback and low-quality case collection usable when fixed Codex tasks are disabled.

**Step 7: Commit validation docs**

```powershell
git add scripts/run-cloudflare-codex-fixed-task-smoke.ps1 docs/validation
git commit -m "test: add fixed Codex task smokes"
```

## Task 9: Parse Structured Fixed-Template Output From Codex Host

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Step 1: Add failing host-agent tests**

Add tests in `crates/codex-host-agent/src/main.rs` or `src/lib.rs`:

- `codex_exec_extracts_static_page_fixed_task_output_from_stdout`
- `codex_exec_rejects_missing_fixed_task_output_for_fixed_template`
- `codex_exec_keeps_process_summary_but_not_raw_stdout`
- `codex_exec_extracts_answer_quality_fixed_task_output_from_stdout`

Use sample stdout containing a final JSON object:

```json
{
  "template_id": "static_page_image2_data_publish",
  "status": "success",
  "artifact": {
    "local_path": "/srv/aiv3/shared/objects/generated-artifacts/database-static-pages/run/page",
    "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/run/page/index.html",
    "manifest_path": "/srv/aiv3/shared/objects/generated-artifacts/database-static-pages/run/page/manifest.json"
  },
  "validation_report": {
    "snapshot_policy": "latest_snapshot_for_state_modules",
    "latest_snapshot": "2026-05-10",
    "source_row_count": 862,
    "current_state_row_count": 851,
    "detail_row_count": 851,
    "unit_policy": "raw_value_checked_then_wan_or_yi",
    "warnings": []
  },
  "source_summary": ["2026-05-10 snapshot"],
  "human_review_reason": null
}
```

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p codex-host-agent codex_exec_extracts_ --lib
```

Expected: tests fail because `codex_exec` currently returns only `CodexHostTaskOutputView` with process/report metadata.

**Step 3: Add fixed-output extraction**

Implement a helper that:

- scans stdout from the end for a valid JSON object;
- accepts JSON wrapped in a `fixed_task_output`, `template_output`, `result`, or `output` envelope;
- requires `template_id` to match the fixed template capability;
- stores the accepted object at `fixed_task_output` in the host output;
- keeps only bounded safe process summaries in `process`.

Do not store raw stdout/stderr in AssistantRun events.

**Step 4: Make Codex command request structured output**

Update `build_codex_command_plan` for fixed templates so the prompt explicitly requires:

- one final JSON object only;
- template-specific output schema;
- no markdown fence around the final object;
- status `needs_human` when the task cannot satisfy no-confirm policy.

If the installed Codex CLI supports a schema flag in this environment, add it through a V3-owned profile setting. If not, keep schema enforcement in post-processing and document that this is a guarded interim path.

**Step 5: Verify platform validation consumes the extracted output**

Add or update `platform-api` tests so `codex_host_fixed_task_transition_audit_event` can validate output nested under:

```json
{
  "fixed_task_output": {
    "template_id": "static_page_image2_data_publish",
    "status": "success"
  }
}
```

Run:

```powershell
cargo test -p codex-host-agent --lib
cargo test -p platform-api codex_host_fixed_task --lib
```

Expected: fixed-template real execution can return a structured result V3 already knows how to validate.

**Step 6: Commit**

```powershell
git add crates/contracts/src/lib.rs crates/codex-host-agent/src crates/platform-api/src/lib.rs
git commit -m "feat: parse fixed Codex Host task outputs"
```

## Task 10: Materialize A V3 Task Bundle For Fixed Codex Runs

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Create or modify: `docs/operations/cloudflare-codex-fixed-task-templates.md`
- Create or modify: `docs/validation/codex-host-task-bundle-smoke.md`

**Step 1: Add failing bundle tests**

Add tests that fixed tasks include a bundle manifest with:

- `task.json` containing the fixed template package;
- `README.md` containing only safe task instructions;
- `schemas/output.schema.json` or an equivalent inline schema snapshot;
- `evidence/summary.json` containing bounded V3-selected scope and source summaries;
- `image2/preview.json` or `image2/preview_asset_key.txt` for static-page tasks when preview is ready.

Run:

```powershell
cargo test -p platform-api codex_host_fixed_task_bundle --lib
```

Expected: tests fail until the bundle manifest exists.

**Step 2: Add bundle metadata to workflow context**

When V3 queues a fixed task, include a safe bundle descriptor in the workflow context:

```json
{
  "fixed_task_bundle": {
    "version": 1,
    "files": [
      {"path": "task.json", "kind": "fixed_task_context"},
      {"path": "README.md", "kind": "instructions"},
      {"path": "schemas/output.schema.json", "kind": "output_schema"}
    ]
  }
}
```

The descriptor may be enough for first pass if the host and V3 share the workflow context. Actual file materialization can be done by the host inside `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT`.

**Step 3: Materialize files in the host workspace**

Before launching Codex, `codex-host-agent` should write:

- `task.json` from the fixed task context;
- `README.md` with fixed instructions and no secrets;
- `output.schema.json` or `expected-output.md`;
- a redacted `runtime.json` with assistant run id, workflow id, capability, template id, and host profile summary.

Never write provider keys, database URLs, raw customer documents, or unrestricted local paths into the task workspace.

**Step 4: Update fixed-task prompt**

Change the prompt from "here is a large JSON package" to:

```text
Use the files in the current task workspace.
Read task.json and expected-output schema.
Return exactly one fixed-template JSON result.
Do not modify files outside the workspace except through explicitly allowed generated-artifacts paths or allowlisted patch files.
```

**Step 5: Verify**

Run:

```powershell
cargo test -p codex-host-agent --lib
cargo test -p platform-api codex_host_fixed_task --lib
```

Expected: fixed tasks can be executed from a bounded workspace bundle, reducing prompt size and improving repeatability.

**Step 6: Commit**

```powershell
git add crates/platform-api/src/lib.rs crates/codex-host-agent/src/lib.rs docs/operations docs/validation
git commit -m "feat: materialize fixed Codex task bundles"
```

## Task 11: Harden Static-Page Fixed-Task Preconditions

**Files:**

- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/operations/static-page-image2-data-publish.md`

**Step 1: Add failing precondition tests**

Add host-agent tests:

- `static_page_template_requires_preview_ready_or_preview_asset_key`
- `static_page_template_requires_dataset_or_database_scope`
- `static_page_template_requires_snapshot_unit_and_detail_policies`
- `static_page_template_rejects_effect_image_confirmation_required_true`

Add platform-api tests:

- third-party static-page initial response has `effect_image_confirmation_required=false`;
- Image2 success without `preview_asset_key` does not enqueue Codex Host;
- Image2 success with `preview_asset_key` enqueues once and deduplicates by `image_job_id`.

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p codex-host-agent static_page_template_requires --lib
cargo test -p platform-api external_channel_static_page --lib
```

Expected: missing preconditions are rejected.

**Step 3: Implement stricter static-page preflight**

Host preflight must require:

- `template_id=static_page_image2_data_publish`;
- `publish_mode=new_generated_artifact_only`;
- `image2.image_job_id` present;
- `image2.visual_contract_status=preview_ready` or a non-empty `preview_asset_key`;
- `human_confirmation_required=false`;
- `policies.effect_image_confirmation_required=false`;
- `policies.continue_to_publish_after_effect_image=true`;
- at least one V3-selected dataset, document, or database source;
- snapshot, trend, unit, and detail-table policies.

V3 should only enqueue the fixed task after Image2 success when a preview asset exists. It may still emit the initial stream/status card immediately.

**Step 4: Verify**

Run:

```powershell
cargo test -p codex-host-agent --lib
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api static_page --lib
```

Expected: the no-confirm static-page path is guarded by Image2 preview readiness and data-scope policy.

**Step 5: Commit**

```powershell
git add crates/codex-host-agent/src/lib.rs crates/platform-api/src/lib.rs docs/operations/static-page-image2-data-publish.md
git commit -m "fix: harden static-page fixed Codex preflight"
```

## Task 12: Register Final Static-Page Artifact And Notify Third-Party Conversation

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs` only if an existing reply/event view lacks a safe status shape.
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Regenerate: `docs/integrations/pure-third-party-integration-guide.zh-CN.html`
- Regenerate: `apps/web/public/external-integrations/pure-third-party-integration-guide.zh-CN.md`
- Regenerate: `apps/web/public/external-integrations/pure-third-party-integration-guide.zh-CN.html`

**Step 1: Add failing final-status tests**

Add platform-api tests:

- completed static-page fixed task with valid `artifact.public_url` appends an AssistantRun event `assistant_run.external_channel_static_page_publish_completed`;
- event includes `draft_id`, `image_job_id`, `codex_host_workflow_execution_id`, `public_url`, and validation summary;
- invalid URLs or missing validation reports produce `codex_host.fixed_task.rejected`;
- final event can be converted to a third-party-visible `artifact_link` or `task_status` reply shape without exposing internals.

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api codex_host_fixed_task --lib
```

Expected: final artifact status is not yet propagated to the originating external conversation.

**Step 3: Implement final completion event**

When `codex_host.fixed_task.completed` validates a `static_page_image2_data_publish` success:

- extract `artifact.public_url`;
- copy or register any needed manifest metadata under V3-generated artifact state if not already represented;
- append `assistant_run.external_channel_static_page_publish_completed`;
- keep `source_refs` from the original draft to reconstruct platform/conversation identity;
- include validation summary, not raw source rows.

Do not overwrite existing customer artifacts. Do not add new third-party request/response fields.

**Step 4: Add third-party status response path**

Use existing response fields:

- `reply.reply_type="artifact_link"` or `task_status`;
- `reply.artifact_links[0].url` as the final V3 generated-artifact URL;
- `reply.card.render_output_id` only when a V3 render output id exists;
- `reply.card.codex_host_workflow_execution_id`;
- `reply.card.validation_summary`.

If the third-party integration uses polling by `assistant_run_id`, make the final event visible through existing status/audit retrieval rather than adding a new public URL.

**Step 5: Update docs**

Document:

- initial stream event shows the Image2/effect-image queue card;
- no customer confirmation is required;
- final artifact link arrives after the fixed publish task completes;
- `codex_host_workflow_execution_id` may be empty at initial queue time and filled in later status/audit events.

Run:

```powershell
npm run build:pure-third-party-guide-html
npm run test:pure-third-party-guide-html
```

**Step 6: Verify**

Run:

```powershell
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api codex_host_fixed_task --lib
node --test .\app\lib\external-integrations.test.mjs
```

Run the Node test from `apps/web`.

Expected: final static-page artifact is customer-visible through existing third-party status surfaces.

**Step 7: Commit**

```powershell
git add crates/platform-api/src/lib.rs crates/contracts/src/lib.rs docs/integrations apps/web/public/external-integrations
git commit -m "feat: publish fixed Codex static-page results to external channels"
```

## Task 13: Add Codex Host Runtime Robustness

**Files:**

- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `crates/workflow-definitions/src/lib.rs` only if heartbeat/cancel states need workflow visibility.
- Modify: `docs/architecture/codex-host-bridge-contract.md`

**Step 1: Add failing robustness tests**

Add tests or integration-level harnesses for:

- timeout turns a long-running command into a failed task with safe reason;
- non-zero exit records safe process summary and no raw stdout/stderr;
- cancellation signal prevents starting a new process or stops a running child when possible;
- task heartbeat updates a bounded event without leaking logs.

**Step 2: Run tests and verify failure**

Run:

```powershell
cargo test -p codex-host-agent --lib
cargo test -p workflow-definitions codex_host --lib
```

Expected: timeout/heartbeat/cancel behavior is not yet fully represented.

**Step 3: Replace blocking `Command::output`**

Use `tokio::process::Command` with:

- configurable timeout, default bounded for private smoke;
- captured stdout/stderr capped at safe byte limits;
- periodic heartbeat event while running;
- explicit failure mapping for launch error, timeout, non-zero exit, parse failure, and cancellation.

**Step 4: Add runtime config**

Support:

```text
CODEX_HOST_AGENT_TASK_TIMEOUT_MS=900000
CODEX_HOST_AGENT_HEARTBEAT_MS=15000
CODEX_HOST_AGENT_STDOUT_LIMIT_BYTES=200000
CODEX_HOST_AGENT_STDERR_LIMIT_BYTES=100000
```

Defaults must be conservative.

**Step 5: Verify**

Run:

```powershell
cargo test -p codex-host-agent --lib
cargo test -p platform-api codex_host_fixed_task --lib
```

Expected: stuck or noisy Codex executions fail safely and remain diagnosable.

**Step 6: Commit**

```powershell
git add crates/codex-host-agent/src crates/workflow-definitions/src/lib.rs docs/architecture/codex-host-bridge-contract.md
git commit -m "feat: harden Codex Host runtime execution"
```

## Task 14: End-To-End Private Smoke For No-Confirm Static Page

Status 2026-05-25: local `static-page-no-confirm` smoke passed; all local fixed-task cases passed; remote `https://v3.elepcloud.com` plan-only readiness passed and mutation remained guarded/skipped. Full mutation smoke still requires explicit operator approval and `-AllowServerMutation`.

**Files:**

- Modify: `scripts/run-cloudflare-codex-fixed-task-smoke.ps1`
- Modify: `docs/validation/static-page-render-smoke.md`
- Modify: `docs/validation/assistant-chat-contract-smoke.md`
- Modify: `docs/operations/cloudflare-codex-fixed-task-templates.md`

**Step 1: Extend smoke cases**

Add an end-to-end static-page case:

1. Submit a third-party complex static-page request using existing public fields.
2. Assert initial response is `task_status=static_page_image2_auto_publish_pending`.
3. Assert stream/status card includes `effect_image_confirmation_required=false`.
4. Advance or wait for Image2 preview to succeed.
5. Assert Codex Host task is queued after preview-ready.
6. Run Codex Host in `plan_only` first.
7. Run Codex Host in `codex_exec` on approved host only after operator review.
8. Assert final output validates and exposes a generated-artifact URL.

**Step 2: Add local plan-only smoke**

Run:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case static-page-no-confirm
```

Expected:

- no 8-server writes;
- fixed task package validates;
- output schema path is exercised;
- rejection paths are tested.

**Step 3: Add private 8-server smoke**

Run only after local plan-only passes and operator approves:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -Case static-page-no-confirm
```

Use `-PlanOnly` for read-only deployment readiness. Use `-AllowServerMutation` only on an approved host after deployment review.

Expected:

- effect image is visible as a stream/status card;
- no customer confirmation is required;
- generated HTML is published under `https://v3.elepcloud.com/generated-artifacts/...`;
- final status includes validation summary and artifact URL;
- no source-code, API, auth, schema, or stable URL changes occur.

**Step 4: Record smoke result**

Update validation docs with:

- date/time;
- environment;
- feature flags;
- Image2 job id;
- Codex Host workflow id;
- final public URL;
- rollback command.

Do not include secrets, raw customer data, or SSH details.

**Step 5: Commit**

```powershell
git add scripts/run-cloudflare-codex-fixed-task-smoke.ps1 docs/validation docs/operations/cloudflare-codex-fixed-task-templates.md
git commit -m "test: smoke no-confirm Codex static-page publishing"
```

## Task 15: Add `data_ingestion_analysis` Fixed Template Contract

Status 2026-05-25: implemented. Contract, host preflight, platform output validation, operations docs, and model-profile allowlist docs are in place. Verified with `cargo test -p contracts data_ingestion --lib`, `cargo test -p codex-host-agent data_ingestion --lib`, `cargo test -p platform-api data_ingestion --lib`, and `cargo test -p platform-api codex_host_fixed_task --lib`.

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/operations/cloudflare-codex-fixed-task-templates.md`
- Modify: `docs/operations/codex-host-model-profiles.md`

**Step 1: Add failing contract tests**

Add contract tests:

- `codex_host_data_ingestion_template_context_round_trips`
- `codex_host_data_ingestion_output_schema_accepts_analysis_ready`
- `codex_host_data_ingestion_output_schema_requires_human_for_production_write`

Run:

```powershell
cargo test -p contracts data_ingestion --lib
```

Expected: tests fail until the template id, example context, and output shape are represented.

**Step 2: Add template id and examples**

Extend fixed template enums and examples with:

- `CodexHostFixedTaskTemplateIdView::DataIngestionAnalysis`
- `CodexHostFixedTaskTemplateContextView::data_ingestion_analysis_example()`
- fixed policies:
  - `mode=read_only_analysis_or_staging_spec`
  - `credential_policy=do_not_request_or_emit_credentials`
  - `production_write_policy=needs_human_confirmation`
  - `public_api_change_allowed=false`
  - `schema_change_allowed_without_confirmation=false`

Do not introduce any third-party public request/response fields for this template.

**Step 3: Add platform output validation**

In `platform-api`, accept `data_ingestion_analysis` output only when:

- `template_id=data_ingestion_analysis`;
- status is `analysis_ready`, `staging_spec_ready`, `needs_human`, or `failed`;
- `source_summary`, `data_quality_report`, and `validation_checks` are present for successful statuses;
- production schema writes, credential requests, or public API changes force `needs_human`;
- no raw credentials or database URLs appear in summary fields.

**Step 4: Add host preflight**

In `codex-host-agent`, fixed-template preflight must reject:

- missing selected source scope;
- `policies.mode` other than `read_only_analysis_or_staging_spec`;
- `credential_policy` other than `do_not_request_or_emit_credentials`;
- production write or schema-change policies that bypass confirmation;
- unapproved host kind for non-dry-run execution.

**Step 5: Verify**

Run:

```powershell
cargo test -p contracts data_ingestion --lib
cargo test -p codex-host-agent data_ingestion --lib
cargo test -p platform-api codex_host_fixed_task --lib
```

Expected: data-ingestion analysis is a first-class fixed template with bounded output validation.

**Step 6: Commit**

```powershell
git add crates/contracts/src/lib.rs crates/codex-host-agent/src/lib.rs crates/platform-api/src/lib.rs docs/operations
git commit -m "feat: define fixed Codex data ingestion template"
```

## Task 16: Queue Data Ingestion Analysis From Customer Requests

Status 2026-05-25: implemented. Existing external chat messages can trigger `data_ingestion_analysis` for data接入/入库/建表/字段映射/schema/ETL/清洗 intent when V3-selected source scope exists and the fixed capability is enabled/allowlisted. Missing source scope returns `data_ingestion_analysis_source_required`; disabled Codex Host falls back to normal V3 behavior. Verified with `cargo test -p platform-api external_channel_data_ingestion --lib`, `cargo test -p platform-api codex_host_fixed_task --lib`, `npm run build:pure-third-party-guide-html`, `npm run test:pure-third-party-guide-html`, and `node --test .\app\lib\external-integrations.test.mjs` from `apps/web`.

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Regenerate: `docs/integrations/pure-third-party-integration-guide.zh-CN.html`
- Regenerate: `apps/web/public/external-integrations/pure-third-party-integration-guide.zh-CN.md`
- Regenerate: `apps/web/public/external-integrations/pure-third-party-integration-guide.zh-CN.html`

**Step 1: Add failing intent tests**

Add platform-api tests:

- customer asks "帮我接入这份表并入库分析字段" -> queues `data_ingestion_analysis`;
- customer asks "这个数据库怎么建表/字段怎么映射" -> queues `data_ingestion_analysis`;
- ordinary answer/question does not trigger data-ingestion Codex task;
- missing selected source scope returns a normal task-status explaining more source material is needed, not a free-form Codex task;
- disabling Codex Host keeps normal V3 answer behavior.

Run:

```powershell
cargo test -p platform-api external_channel_data_ingestion --lib
```

Expected: tests fail until intent detection and package builder exist.

**Step 2: Implement intent detection using existing fields**

Detect from existing prompt/message content and selected scope only. Trigger words may include:

- `数据接入`
- `入库`
- `建表`
- `字段映射`
- `数据源`
- `同步`
- `数据库分析`
- `schema`
- `ETL`
- `导入`
- `清洗`

Do not add new public request fields, URLs, auth, or third-party response fields. Use existing `reply.task_status`, `reply.card`, and AssistantRun events.

**Step 3: Build fixed package**

Package only:

- assistant run id;
- customer goal and inferred intent;
- tenant id and V3-selected source/document/file/database ids;
- bounded schema/sample summaries already available in V3 context;
- source visibility and missing-evidence status;
- fixed policies forbidding credential emission, public API changes, and production schema writes without confirmation.

Do not place raw database URLs, credentials, full table dumps, or unrestricted local paths in the task package.

**Step 4: Enqueue fixed task when enabled**

When `CODEX_HOST_TASK_ENABLED=true` and allowlist contains `data_ingestion_analysis`:

- create a Codex Host workflow execution;
- append `assistant_run.data_ingestion_analysis_queued`;
- return or record a task-status card using existing fields:
  - `reply.reply_type=task_status`;
  - `reply.task_status=data_ingestion_analysis_queued`;
  - `reply.card.type=v3_data_ingestion_analysis`;
  - `reply.card.codex_host_workflow_execution_id`.

If disabled, return normal V3 behavior or a bounded accepted/status reply.

**Step 5: Update third-party docs**

Document that customers can ask for data接入/入库分析 in natural language through the existing chat/event endpoint. Emphasize:

- no new public fields;
- V3-selected source scope controls what Codex receives;
- read-only analysis/staging specs may run automatically;
- credentials, production schema writes, public API changes, and deployment need human confirmation.

Run:

```powershell
npm run build:pure-third-party-guide-html
npm run test:pure-third-party-guide-html
```

**Step 6: Verify**

Run:

```powershell
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api codex_host_fixed_task --lib
node --test .\app\lib\external-integrations.test.mjs
```

Run the Node test from `apps/web`.

Expected: a customer request can queue data-ingestion analysis without changing third-party API shape.

**Step 7: Commit**

```powershell
git add crates/platform-api/src/lib.rs docs/integrations apps/web/public/external-integrations
git commit -m "feat: queue fixed Codex data ingestion analysis"
```

## Task 17: Smoke Data Ingestion Analysis And Safe Staging Boundaries

Status 2026-05-25: implemented for local plan-only and read-only 8-server readiness. `data-ingestion-analysis` smoke passed locally and remote mutation remains guarded. Full private mutation smoke still requires explicit operator approval and `-AllowServerMutation`.

**Files:**

- Modify: `scripts/run-cloudflare-codex-fixed-task-smoke.ps1`
- Create or modify: `docs/validation/data-ingestion-analysis-smoke.md`
- Modify: `docs/operations/cloudflare-codex-fixed-task-templates.md`

**Step 1: Add smoke cases**

Add cases:

- file/table profiling request -> `analysis_ready`;
- field mapping request -> `analysis_ready` with mapping confidence;
- already configured source + new staging target request -> `staging_spec_ready`;
- new credential request -> `needs_human`;
- production table overwrite/schema migration request -> `needs_human`;
- public API/integration change request -> `needs_human`.

**Step 2: Run local plan-only smoke**

Run:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case data-ingestion-analysis
```

Expected:

- no real database writes;
- no credentials in package/output;
- structured output validates;
- unsafe cases become `needs_human`.

**Step 3: Run private 8-server smoke after approval**

Run only after local plan-only passes:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -Case data-ingestion-analysis
```

Expected:

- Codex Host runs on approved host kind;
- output contains data-quality report, mapping plan, validation checks, and recommended next actions;
- no production schema/write/public API changes occur;
- runtime audit contains `codex_host.fixed_task.completed` or `needs_human`.

**Step 4: Record smoke result**

Update validation docs with:

- date/time;
- source type;
- workflow id;
- status;
- risk decision;
- rollback flags.

Do not include credentials, database URLs, raw customer data dumps, or SSH details.

**Step 5: Commit**

```powershell
git add scripts/run-cloudflare-codex-fixed-task-smoke.ps1 docs/validation docs/operations/cloudflare-codex-fixed-task-templates.md
git commit -m "test: smoke fixed Codex data ingestion analysis"
```

## Recommended Rollout Policy

Initial production setting:

```text
static_page_image2_data_publish:
  confirmation: not required for new generated artifact after Image2 preview-ready
  execution: plan_only until structured fixed-output parsing passes
  codex_exec: enabled only after one successful private 8-server smoke
  publish: allowed only under /generated-artifacts/

answer_quality_autofix:
  confirmation: not required for diagnosis and patch proposal
  auto-apply: disabled until structured output, allowlist validation, and 5 consecutive low-risk cases pass tests
  deploy: manual

data_ingestion_analysis:
  confirmation: not required for read-only analysis and staging/spec proposals from customer requests
  execution: plan_only until fixed output parsing and source-scope validation pass
  codex_exec: enabled only after private smoke proves no credential/schema/public API leakage
  production writes: human confirmation required
```

After stability:

```text
static_page_image2_data_publish:
  keep no-confirm for new artifacts
  still require confirmation for overwrite/stable URLs/customer send

answer_quality_autofix:
  allow auto-apply for low-risk allowlisted patches with tests
  keep medium/high risk and deploy behind human exception review

data_ingestion_analysis:
  allow automatic analysis/spec generation for selected sources
  allow V3-managed staging job specs only for already configured sources
  keep credentials, production schema changes, schedule changes, and public API changes behind human exception review
```

## Success Criteria

- V3 can queue all approved fixed Cloudflare Codex templates from server-owned policy, not arbitrary user free text.
- Static-page advanced flow produces Image2-first, real-data-bound,口径-validated generated artifacts with no per-task confirmation for new artifact publication.
- Low-quality answers are passively collected and routed to `answer_quality_autofix` without blocking normal customer replies.
- Answer-quality auto-optimization cannot touch files outside the allowlist and must include regression tests.
- Customer-requested data接入/入库/数据库分析 can queue `data_ingestion_analysis` through existing chat/event fields, with V3-selected source scope and no credential leakage.
- Data-ingestion analysis can produce mapping plans, data-quality reports, validation checks, and staging specs while keeping production schema writes and public API changes behind human confirmation.
- Runtime/audit surfaces show template id, status, validation summary, and rollback pointers.
- Disabling Codex Host returns V3 to direct static-page and normal answer behavior.
