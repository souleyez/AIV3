# V3 Cloudflare Codex Fixed Escalation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let V3 safely delegate two fixed advanced workflows to Cloudflare Codex without per-task human confirmation: Image2-first static-page publishing and low-quality answer auto-optimization.

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
allowed_capabilities = ["static_page_image2_data_publish", "answer_quality_autofix"]
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
rg -n "static_page_image2_data_publish|answer_quality_autofix|cloudflare-codex-fixed-tasks" docs
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
- `codex_exec_rejects_untemplated_write_capability`
- `static_page_template_requires_generated_artifact_publish_mode`
- `answer_quality_template_rejects_non_allowlisted_file_scope`

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

## Recommended Rollout Policy

Initial production setting:

```text
static_page_image2_data_publish:
  confirmation: not required for new generated artifact
  execution: enabled after one successful private smoke
  publish: allowed only under /generated-artifacts/

answer_quality_autofix:
  confirmation: not required for diagnosis and patch proposal
  auto-apply: disabled until 5 consecutive low-risk cases pass tests
  deploy: manual
```

After stability:

```text
static_page_image2_data_publish:
  keep no-confirm for new artifacts
  still require confirmation for overwrite/stable URLs/customer send

answer_quality_autofix:
  allow auto-apply for low-risk allowlisted patches with tests
  keep medium/high risk and deploy behind human exception review
```

## Success Criteria

- V3 can queue both fixed Cloudflare Codex templates from server-owned policy, not user free text.
- Static-page advanced flow produces Image2-first, real-data-bound,口径-validated generated artifacts with no per-task confirmation for new artifact publication.
- Low-quality answers are passively collected and routed to `answer_quality_autofix` without blocking normal customer replies.
- Answer-quality auto-optimization cannot touch files outside the allowlist and must include regression tests.
- Runtime/audit surfaces show template id, status, validation summary, and rollback pointers.
- Disabling Codex Host returns V3 to direct static-page and normal answer behavior.
