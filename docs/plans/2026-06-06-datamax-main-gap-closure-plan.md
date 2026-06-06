# DataMax Main Gap Closure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the remaining major DataMax production gaps with a controlled, testable sequence: final 8-server release smoke, Xinbai report/template stability, safe historical enrichment, operator model-gateway validation, data-source row identity decisions, and passive answer-quality recovery.

**Architecture:** DataMax remains the system of record for tenants, dataset/document scopes, parsed evidence, deterministic facts, generated artifacts, workflow state, model profiles, and third-party authorization. The plan adds validation, internal-only operators, safe background jobs, and template governance around existing flows; it must not change third-party public URLs, authentication, required request fields, or existing response fields. Model calls stay stateless and receive only DataMax-scoped temporary evidence.

**Tech Stack:** Rust `platform-api`, `storage`, `ingest-worker`, `retrieval-worker`, `static-page-worker`, `codex-host-agent`; PostgreSQL; workflow task queues; Next.js `apps/web`; existing smoke scripts under `scripts/`; 8-server systemd services; DataMax/Image2/static-page template pipeline.

---

## Baseline - 2026-06-06

- Before the Task 5/6 documentation pass, local `main` and 8 server `/srv/aiv3/repo` were both at `7be4111eb85a` after the current-head report-link fix, scoped-document smoke addition, external scoped-memory isolation fix, attachment-title scope supply fix, smoke additions, and validation updates.
- 8 server `/srv/aiv3/repo` has pulled `7be4111eb85a`; `platform-api` release build/restart passed after the platform-code fixes. Script/doc-only commits were pulled without service restart when no deployed binary changed.
- 8 server has a pre-existing untracked `mode` file. Leave it untouched.
- P0 flows already have recent passing receipts in `docs/validation/datamax-main-gap-closure.md`:
  - third-party ordinary 20-way chat;
  - main-site 20-way chat;
  - main-site AssistantRun live streaming;
  - third-party progress/artifact streaming;
  - static-page 5-way;
  - Cloudflare fallback concurrency guard;
  - Xinbai report link/export smoke;
  - current-head Xinbai report JSON/SSE smoke with a required clickable text link;
  - current-head static-page 5-way smoke;
  - current-head third-party scoped-document and attachment-title smoke;
  - data-ingestion staging plan and confirmed sync;
  - post-ingest deterministic enrichment for newly parsed documents;
  - low-load static-page template prewarm private smoke.
- Still open:
  - historical backfill has only a summary-only dry-run receipt after commit `8f835d0`; no real backfill batch has been approved or run;
  - authenticated model-gateway operator smoke still needs a legitimate operator cookie or local-key login;
  - full existing-document enrichment backfill remains disabled by policy;
  - source-row completeness for `bi_contract_warning` and `bi_rentsales_detail` needs a business decision before row-level remapping;
  - low-quality answer recovery is intentionally passive/safe-disabled until an operator rollout decision;
  - Xinbai modular report should remain the only default template for that project, with old templates excluded from normal matching;
  - model-visible platform capabilities should be made explicit enough that report generation, document ingestion, deep parsing, data ingestion, and proactive messaging can be routed without leaking tool traces or blocking normal answers;
  - public third-party integration docs must be checked after every additive field or report-card/export-field behavior change.

## Non-Negotiable Rules

- Do not change third-party public URLs, auth, required request fields, or existing response fields unless the operator explicitly approves.
- Additive response fields are allowed only when old clients can ignore them.
- Do not touch 120 server.
- Use 8 server as the production/demo validation target.
- Do not re-enable a hard customer-facing answer-quality gate.
- Do not let Codex executor mutate datasets, permissions, credentials, deployments, or public integration contracts.
- Do not print raw credentials, database URLs, provider payloads, full customer documents, raw table rows, or bearer/session tokens in prompts, logs, docs, or artifacts.
- VLM reparse is premium and budget-gated. Use it only after cheaper parse/enrichment recovery fails or an operator explicitly approves.
- Static-page high-quality generation remains Image2-first. Accepted templates may be reused immediately while a background refresh continues.
- Third-party temporary documents, dataset scopes, and conversation-level authorization are authoritative.

## Active Execution Sheet

| Priority | Gap | Current State | Next Action | Done When |
| --- | --- | --- | --- | --- |
| P0 | Final 8-server release gate | Current-head `15756668bcad` release-gate rerun passed third-party ordinary 20-way, AssistantRun live streaming, static-page 5-way, Xinbai report/export, third-party scoped-document chat, document-quality regression, and service-status checks. Main-site chat-session 20-way remains auth/scope-dependent and failed without a legitimate main-system session; Cloudflare/model-gateway guard remains pending without operator credentials. | Provide a legitimate main-system session/operator credential, then rerun main-site 20-way and authenticated model-gateway/Cloudflare guard; keep the passing smokes in the gate. | All required smokes pass on the deployed commit; service status and rollback note are recorded. |
| P0 | Xinbai report/template contract | Current-head report/export regression is fixed: JSON and SSE both expose one clickable text report link plus export files. | Keep the smoke in the release gate and fix only new regressions. | One clickable report link, correct focus, normal answer not truncated, exports accessible, no false report trigger. |
| P0 | Third-party scope and temporary attachments | Current-head 8-server smoke passed: dataset group + explicit external doc union, same-conversation follow-up restore, changed conversation isolation, and attachment-title scoped document answer. | Keep `smoke:external-scoped-document-chat` in the release gate and fix only new regressions. | Temporary documents and grouped datasets remain authorized across the same conversation; unrelated sessions cannot see them. |
| P1 | Historical enrichment/backfill | New documents enrich; full historical backfill disabled. 8-server `--summary-only --dry-run` passed after `8f835d0`. | Plan a one-dataset/one-kind dry-run before any real batch; keep production backfill disabled until reviewed. | No document titles/content printed; no duplicate counts; no production backfill until reviewed. |
| P1 | Authenticated model-gateway operator smoke | Current-head `15756668bcad` unauthenticated guard passes with `401 auth_session_required`; checked env still has no operator smoke cookie/email/local-key. | Run `smoke:model-gateway-operator` with a legitimate operator session or local-key login. | Operator status/profile health is proven without bypasses or leaked secrets. |
| P1 | Data-source row identity | Current-head 8-server live audit passed and confirms question/report readiness, but `bi_contract_warning` and `bi_rentsales_detail` collapse 193 latest-sync rows under current identity. Decision memo `docs/operations/data-source-row-identity-decision.md` now fixes the safe boundary: production stays unchanged, row-level semantics require staging-only discriminator validation first. | Get the business decision: entity/latest-snapshot semantics vs row-level detail. If row-level is required, test staging-only discriminator mapping before production. | Report completeness semantics are documented and validated before production mapping changes. |
| P1 | Passive low-quality recovery | Current-head `1e99281ff695` audit confirms the customer-facing hard gate is unset, the dedicated live-autofix flag is unset, and `answer_quality_autofix` is absent from both task/capability allowlists. Local answer-quality regressions pass, so the safe-disabled state is proven rather than assumed. | Keep disabled; only enable passive collection/manual review after an explicit operator decision. Do not create live autofix Codex tasks until the dedicated flag and both allowlists are intentionally set. | Weak answers are visible for review without suppressing normal customer answers, and no live autofix task can be created accidentally. |
| P2 | Template library hygiene | Accepted Xinbai modular report should dominate matching; old artifacts exist historically. | Exclude old/non-default templates from normal matching; clean local generated artifacts only after explicit approval. | Same project/scope/default prompt reuses the accepted modular template; old dark/legacy templates do not pollute links. |
| P2 | Model-visible capability routing | Current head has an internal capability catalog for report/static-page, document processing, data ingestion, collection/integration setup, and proactive message routing. Temporary template/contract/traffic-stat report materials are now routed to the report/static-page workflow instead of being treated as permanent document-processing tasks; selected 8-server smoke passed. | Keep the full fixture in the release gate, add new customer phrasing as fixtures, and audit docs after any additive artifact/report behavior change. | The model can choose supported platform workflows while the platform keeps auth/scope/tool execution authoritative, normal answers continue, and no public tool traces leak. |
| P2 | Third-party contract docs | Current-head docs and public copies were audited on 2026-06-06: DataMax naming is explicit, legacy `v3_*`/`X-V3-*` protocol names are documented as compatibility fields, dataset/document union scope and same-conversation authorization are covered, and report exports name `table-data.csv`, `report.ppt`, and `report.md`. | Re-audit and regenerate public copies after any customer-visible report/artifact additive behavior change. | Third parties can understand dataset/document union scope, persisted conversation authorization, template reference uploads, report link fields, and export files without re-integration. |
| P2 | Observability and runbooks | Operator page and validation docs exist. | Keep runbooks aligned with the current release gate and failures. | Operators can diagnose queue/model/report/enrichment/data-ingestion health without raw logs. |

---

## Remaining Execution Order

Use this order unless a live regression forces a narrower hotfix:

1. **P0 release gate:** keep 8-server smoke current for third-party ordinary chat, Xinbai report/export, static-page 5-way, main-site 20-way, main-site streaming, and document-quality regression. Auth-dependent main-site/operator checks can be recorded as pending only when legitimate credentials are unavailable.
2. **P0 report/static-page stability:** keep the Xinbai modular monthly report as the default template, verify one clickable report link, export files, focus routing, no answer truncation, and no false trigger for metric-definition/resume questions.
3. **P0 third-party scope:** keep dataset/document union, same-conversation authorization reuse, changed-conversation isolation, and temporary attachment-title supply in every release gate.
4. **P1 model gateway:** run the authenticated operator smoke with a legitimate operator session or local-key login; do not bypass auth for validation.
5. **P1 data-source identity:** decide whether collapsed tables remain entity-level facts or need row-level materialization; test any discriminator change in staging before production mapping changes.
6. **P1 historical enrichment:** keep summary-only dry-run as the default; only run a tiny real batch after reviewing dataset, enrichment kind, queue load, and rollback behavior.
7. **P1 passive answer-quality recovery:** keep hard gates disabled; collect weak-answer candidates for manual review or explicitly approved passive Codex tasks only.
8. **P2 template hygiene:** exclude stale Xinbai templates from normal matching; clean generated artifacts only after explicit approval and path verification.
9. **P2 capability routing and docs:** keep the model-visible capability catalog synchronized with fixtures and public docs, while preserving all third-party public URLs/auth/request fields/existing response fields.

## Task 1: Current-Head 8-Server Release Gate

**Files:**

- Update: `docs/validation/datamax-main-gap-closure.md`
- Inspect only if smoke fails: `crates/platform-api/src/lib.rs`
- Inspect only if smoke fails: `crates/static-page-worker/src/main.rs`
- Inspect only if smoke fails: `apps/web/app/HomePageClient.js`
- Inspect only if smoke fails: `scripts/smoke/*.mjs`

**Step 1: Confirm local worktree**

Run:

```powershell
git status --short --branch
git rev-parse --short=12 HEAD
git diff --check
```

Expected:

- branch is `main`;
- no unrelated dirty files;
- head is the commit intended for 8 server;
- `git diff --check` has no whitespace errors.

**Step 2: Confirm 8-server code and services**

Run:

```powershell
ssh 8服务器 "set -e; cd /srv/aiv3/repo; git pull --ff-only; git rev-parse --short=12 HEAD; git status --short --branch; systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-static-page-worker.service aiv3-document-enrichment-worker.service aiv3-ingest-worker.service aiv3-retrieval-worker.service"
```

Expected:

- 8 server head matches local/pushed head;
- status shows only the known untracked `mode`;
- listed services are `active`.

**Step 3: Run production/private smoke set**

Run on the environment that has the private third-party bearer configured:

```powershell
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20 --timeout-ms 120000
npm run smoke:main-chat-20way -- --base-url https://v3.elepcloud.com --concurrency 20 --timeout-ms 90000 --poll-timeout-ms 120000
npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com --timeout-ms 120000 --strict-deltas
npm run smoke:static-page-5way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 5 --timeout-ms 120000 --poll-timeout-ms 300000
npm run smoke:cloudflare-fallback-2way -- --base-url https://v3.elepcloud.com --max-allowed 2 --min-expected 2
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --timeout-ms 180000
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local
```

Expected:

- third-party ordinary Q&A works under 20 concurrent requests;
- main-site chat returns assistant messages under 20 concurrent requests;
- main-site live stream emits real deltas and no duplicate final text;
- static-page explicit requests return artifact links;
- Cloudflare fallback concurrency remains bounded;
- Xinbai report/export returns one report surface and accessible files;
- document-quality regression still covers one-character PDF, `邓工是谁`, elderly-care procedures, resume aggregation, attendance table, smart-home, and smart-elevator cases.

**Step 4: Record validation**

Append to `docs/validation/datamax-main-gap-closure.md`:

- deployed commit;
- command list;
- receipt paths;
- pass/fail table;
- service status;
- known server dirty state (`mode` only);
- rollback note.

**Step 5: Commit**

```powershell
git add docs/validation/datamax-main-gap-closure.md
git commit -m "Record DataMax current-head release gate"
git push
```

---

## Task 2: Safe Historical Enrichment And Fingerprint Backfill Audit

**Files:**

- Modify only if a bug is found: `crates/platform-api/src/bin/document-fingerprint-backfill.rs`
- Modify only if a bug is found: `crates/storage/src/lib.rs`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Run local binary regression**

Run:

```powershell
cargo fmt --check -p platform-api
cargo test -p platform-api --bin document-fingerprint-backfill
cargo check -p platform-api
git diff --check
```

Expected:

- `--summary-only` tests pass;
- default behavior stays compatible;
- no manual formatting changes are needed.

**Step 2: Run 8-server summary-only dry-run**

Run:

```powershell
$remote = @'
set -e
cd /srv/aiv3/repo
set -a
. /etc/aiv3/aiv3.env
set +a
./target/release/document-fingerprint-backfill --limit 20 --dry-run --summary-only --pretty
'@
ssh 8服务器 $remote
```

Expected:

- output contains aggregate fields such as `candidate_count`, `dry_run`, `summary_only`, `document_report_count`, `would_record_count`, `skipped_count`, `duplicate_count`, and `recorded_count`;
- output does not contain a `documents` array;
- no document title, content, local path, external URL, database URL, or credential is printed.

**Step 3: Decide whether to run a tiny real batch**

Do not run a real batch by default. If the operator approves:

- choose one low-risk dataset;
- choose one enrichment kind;
- keep batch size under 20;
- run during low-load window;
- capture before/after counts only.

Expected:

- no upload/chat degradation;
- no duplicate fact inflation;
- queue backlog returns to normal;
- rollback is "disable worker/backfill and leave generated summaries unused", not deleting customer documents.

**Step 4: Record validation**

Append:

- 8-server coverage counts;
- summary-only dry-run result;
- whether real batch remains disabled;
- next reviewed batch proposal if needed.

---

## Task 3: Xinbai Modular Report Contract

**Files:**

- Inspect/modify if regression appears: `crates/platform-api/src/lib.rs`
- Inspect/modify if regression appears: `crates/static-page-worker/src/main.rs`
- Inspect/modify if regression appears: `apps/web/app/HomePageClient.js`
- Inspect/modify if regression appears: `scripts/smoke/external-report-export.mjs`
- Update: `docs/validation/external-report-export-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Re-run explicit report smoke**

Run:

```powershell
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --timeout-ms 180000
```

Required assertions:

- report card title is `新世界百货经营管理月报表`;
- reply text contains at most one primary report link;
- link is clickable and uses the generated-artifact URL;
- `artifact_links` or equivalent existing artifact field carries the same primary report URL for third-party clients;
- `table-data.csv`, `report.ppt`, and `report.md` are accessible;
- normal model answer is not truncated when report generation/reuse is triggered.

**Step 2: Verify domain-scoped trigger behavior**

Use the smoke script or add cases to it:

- should trigger report: `取高`, `经营状况`, `风险识别`, `销售缺口`, `助推`, `看看整体经营情况`, `看看XX店经营风险`;
- should not trigger report: `取高是什么意思？`, `风险识别系统有哪些项目经历？`;
- should preserve answer plus report: questions that ask for an explanation and also imply report generation.

Expected:

- broad trigger terms are scoped to Xinbai/database-report context only;
- general knowledge or resume/project-experience questions do not generate report artifacts.

**Step 3: Verify template behavior**

Required behavior:

- Xinbai project defaults to the latest modular monthly report template;
- old dark/legacy/fallback templates are not chosen by normal matching;
- same dataset/default_prompt/focus reuses the accepted template first;
- if user asks for style change, refresh happens on the existing accepted template lineage;
- if data changes, page data refreshes without forcing a new visual template.

**Step 4: Fix only failed assertions**

Do not broaden global report routing. Any new trigger expansion should live in the Xinbai/domain-specific routing policy.

**Step 5: Commit**

```powershell
git add crates apps/web scripts docs/validation
git commit -m "Stabilize Xinbai report template contract"
git push
```

---

## Task 4: Third-Party Scoped Documents And Temporary Attachments

**Files:**

- Inspect/modify if regression appears: `crates/platform-api/src/lib.rs`
- Inspect/modify if regression appears: `crates/storage/src/lib.rs`
- Inspect/modify if regression appears: `scripts/smoke/external-channel-20way.mjs`
- Add if missing: `scripts/smoke/external-scoped-document-chat.mjs`
- Update: `docs/validation/document-understanding-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Add or run scoped-document smoke**

Cover these cases:

- third-party sends `dataset_external_ids`;
- third-party sends explicit document IDs outside the dataset;
- repeated follow-up in the same `conversation_external_id` reuses prior authorization;
- a temporary uploaded attachment is included in the current answer scope;
- changing `conversation_external_id` drops prior authorization.

Expected:

- if a document belongs to an already authorized dataset, document-level duplication is ignored;
- if a document is outside authorized datasets but explicitly authorized, it is included;
- no other conversation can see the temporary document.

**Step 2: Include high-value customer questions**

Use safe scoped fixtures where available:

- resume timeline and project-experience aggregation;
- elderly-care procedure questions;
- attendance date/work-hour questions;
- Xinbai report context questions.

Expected:

- answer uses scoped evidence and deterministic facts where available;
- no "remote API unavailable" response when the platform path can answer or truthfully queue work;
- no raw tool trace is exposed.

**Step 3: Commit**

```powershell
git add crates scripts docs/validation
git commit -m "Cover third-party scoped document chat"
git push
```

---

## Task 5: Authenticated Model-Gateway Operator Smoke

**Files:**

- Modify only if smoke reveals a bug: `scripts/smoke/model-gateway-operator.mjs`
- Modify only if smoke reveals a bug: `crates/platform-api/src/lib.rs`
- Modify only if smoke reveals a bug: `apps/web/app/lib/model-gateway.js`
- Update: `docs/operations/model-gateway-rollout.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Run unauthenticated guard**

Run:

```powershell
npm run smoke:model-gateway-operator -- --base-url https://v3.elepcloud.com --allow-missing-credentials --output-dir target/model-gateway-operator-smoke-no-credentials
```

Expected:

- unauthenticated status returns `401 auth_session_required`;
- missing credentials are recorded as pending, not pass.

**Step 2: Run with a legitimate operator session**

Use exactly one existing auth path:

```powershell
npm run smoke:model-gateway-operator -- --base-url https://v3.elepcloud.com --cookie "<existing aidp_v3_session cookie>"
```

or:

```powershell
$env:MODEL_GATEWAY_OPERATOR_SMOKE_EMAIL="<operator email>"
$env:MODEL_GATEWAY_OPERATOR_SMOKE_LOCAL_KEY="<operator local key>"
npm run smoke:model-gateway-operator -- --base-url https://v3.elepcloud.com
```

Expected:

- status/profile calls pass;
- Right `gpt-5.5` assistant profile is visible and configured for concurrency 20;
- MiniMax fallback remains bounded;
- report contains no cookie, local key, provider key, raw auth env name, or provider payload.

**Step 3: Optional provider probe**

Only after explicit approval:

```powershell
npm run smoke:model-gateway-operator -- --base-url https://v3.elepcloud.com --cookie "<existing aidp_v3_session cookie>" --run-profile-test
```

Expected:

- sanitized provider test succeeds or fails truthfully;
- no secret is printed.

---

## Task 6: Data-Source Row Identity Decision

**Files:**

- Inspect/modify if mapping changes: `crates/ingest-worker/src/main.rs`
- Inspect/modify if mapping changes: `crates/retrieval-worker/src/main.rs`
- Inspect/modify if mapping changes: `crates/platform-api/src/lib.rs`
- Update: `docs/validation/data-ingestion-staging-sync-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Produce sanitized current report**

Run:

```powershell
bash scripts/run-data-ingestion-staging-live-smoke.sh
```

Expected report fields only:

- source id;
- table name;
- configured identity columns;
- source row count;
- unique materialized document count;
- collapsed duplicate row count;
- recommended action.

Do not print raw rows, credentials, URLs, or customer table contents.

**Step 2: Decide semantics**

For `bi_contract_warning` and `bi_rentsales_detail`:

- if entity/store-level aggregation is intended, keep current mapping and document it;
- if row-level report completeness is required, test a staging-only mapping with finer discriminator columns.

**Step 3: Verify any mapping change**

Run:

```powershell
cargo fmt --check -p ingest-worker -p retrieval-worker
cargo test -p ingest-worker --bin ingest-worker external_source_ingest_table_counts -- --nocapture
cargo test -p retrieval-worker --bin retrieval-worker external_index_document_ids -- --nocapture
cargo check -p ingest-worker -p retrieval-worker
bash scripts/run-data-ingestion-staging-live-smoke.sh
```

Expected:

- source rows, unique documents, chunks, and evidence are counted separately;
- Xinbai report figures are not overstated or silently collapsed.

---

## Task 7: Passive Low-Quality Recovery

**Files:**

- Inspect/modify if enabling collection: `crates/platform-api/src/lib.rs`
- Inspect/modify if enabling collection: `crates/codex-host-agent/src/lib.rs`
- Update: `docs/operations/answer-quality-autofix.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Confirm safe-disabled production state**

Run a sanitized env/status audit on 8 server and record only booleans:

- `ASSISTANT_RUN_ANSWER_QUALITY_GATE_ENABLED` is unset or false;
- `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED` is unset or false;
- `CODEX_HOST_TASK_ALLOWLIST` does not include `answer_quality_autofix`;
- `CODEX_HOST_CAPABILITY_ALLOWLIST` does not enable that capability.

Expected:

- no normal customer answer is blocked;
- no live autofix Codex task can be created.

Current-head audit, 2026-06-06:

- deployed repo: `1e99281ff695`;
- `aiv3-platform-api.service`: active;
- `aiv3-codex-host-agent.service`: active;
- `ASSISTANT_RUN_ANSWER_QUALITY_GATE_ENABLED=true`: no;
- `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED=true`: no;
- `CODEX_HOST_TASK_ENABLED=true`: yes;
- `CODEX_HOST_TASK_ALLOWLIST` is set but does not include `answer_quality_autofix`;
- `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES` is set but does not include `answer_quality_autofix`;
- result: live `answer_quality_autofix` execution remains impossible under current 8-server configuration, and normal answers cannot be blocked by the rolled-back gate.

**Step 2: If enabling later, enable passive collection first**

Allowed first rollout:

- collect weak-answer signal;
- classify root cause as missing evidence, parse defect, report/data defect, model-composition defect, or unsupported request;
- show operator queue;
- require manual review before code changes;
- do not suppress or replace customer answer.

Verification:

```powershell
cargo test -p platform-api answer_quality --lib
cargo test -p platform-api assistant_run_answer_quality_gate --lib
cargo test -p codex-host-agent answer_quality --lib
powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local
```

Latest verification, 2026-06-06:

- `cargo test -p platform-api answer_quality_autofix --lib` passed, 14 tests;
- `cargo test -p platform-api answer_quality --lib` passed, 35 tests;
- `cargo test -p platform-api assistant_run_answer_quality_gate --lib` passed, 14 tests;
- `cargo test -p codex-host-agent answer_quality --lib` passed, 3 tests;
- `powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local` passed with receipt `target/document-quality-smoke/document-quality-smoke-20260606T111618Z-11416.json`.

---

## Task 8: Template Library Hygiene

**Files:**

- Inspect/modify if matching is wrong: `crates/platform-api/src/lib.rs`
- Inspect/modify if matching is wrong: `crates/storage/src/lib.rs`
- Update: `docs/validation/static-page-render-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Audit accepted templates**

Produce a sanitized report with:

- template id;
- project/dataset scope hash or count;
- default prompt hash or label;
- accepted/current/default flags;
- last used time;
- artifact URL only when already public.

Do not print customer raw data or local source paths.

**Step 2: Enforce Xinbai default matching**

Required behavior:

- latest modular monthly Xinbai report is the only default for Xinbai normal matching;
- old dark, fallback, and experimental reports are excluded unless explicitly selected;
- dataset overlap plus same `default_prompt` can reuse an accepted template;
- focus changes reorder modules rather than creating a new visual template by default.

**Step 3: Cleanup only with explicit approval**

If deleting local generated artifacts is requested, use the local backup-first deletion helper for unclear/source-like files. Rebuildable target artifacts may be cleaned only after path verification and only within the intended workspace.

---

## Task 9: Model-Visible Platform Capability Routing

**Files:**

- Inspect/modify: `crates/platform-api/src/lib.rs`
- Inspect/modify: `crates/platform-api/src/assistant_capabilities.rs`
- Inspect/modify if present: `crates/assistant-runtime/src/*.rs`
- Inspect/modify: `scripts/run-external-capability-routing-smoke.ps1`
- Inspect/modify: `scripts/smoke/external-report-export.mjs`
- Update: `docs/validation/external-capability-routing-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Inventory model-visible capabilities**

Produce a small internal capability table, not customer-visible copy, covering:

- ordinary answer composition from scoped retrieval/facts;
- document upload/parse/index status;
- deterministic post-ingest enrichment and deep parsing;
- VLM reparse as premium fallback only;
- third-party dataset/document/conversation authorization;
- report/static-page generation and accepted-template reuse;
- data-source connection analysis and staging plan;
- collection/connector tasks;
- proactive outbound message/task notification;
- operator escalation / human confirmation when required.

Expected:

- capabilities describe what the model may request, not what the model may execute directly;
- platform-side scope/auth/queue policy remains authoritative;
- no raw tool schema, secret, provider payload, or internal credential path is exposed to customers.

Current 2026-06-06 state:

- `external_channel_model_tool_capability_guidance_lines()` exposes a safe internal capability catalog for ordinary answer composition, static-page/report artifacts, document processing, data ingestion analysis, collection/integration setup, and proactive message routing.
- The model may request a platform workflow, but DataMax still owns auth, scope, queue policy, artifact publication, and customer-visible response shaping.
- Temporary templates, contracts, traffic-stat files, and other report reference materials are explicitly described as scoped answer/report material, not automatic permanent dataset changes.
- VLM/deep reparse remains a premium fallback and is not selected by this report-material routing path.

**Step 2: Tighten routing prompts and guards**

Add or update routing guidance so these customer expressions can choose a workflow without cutting off the normal answer:

- Xinbai report domain: `取高`, `经营状况`, `经营健康度`, `整体经营情况`, `风险识别`, `销售缺口`, `助推`, `哪些门店需要关注`;
- document domain: `分析附件`, `按这个模板`, `临时上传合同`, `客流统计`, `重新解析`, `看上传文件`;
- data domain: `接入数据库`, `建表`, `字段映射`, `把数据入库`, `生成经营报表`;
- proactive domain: `完成后通知`, `需要主动发消息`, `让系统继续处理`.

Expected:

- normal answer continues even when a report/static-page task is queued;
- third-party clients receive artifact links through existing report/artifact fields;
- broad routing stays domain-scoped and does not turn generic questions like `取高是什么意思？` or resume project-experience questions into report tasks.

Current 2026-06-06 state:

- Xinbai report wording now includes `坪效`, `门店面积`, `合同面积`, `客流统计`, `客流数据`, `客流同比`, `客流月同比`, `客流年同比`, and common update verbs such as `补充`, `增加`, `加上`.
- Focus routing maps contract/area/坪效/traffic/rent-sales/health terms to `经营总览` before the generic contract-detail mapping.
- Accepted template-baseline links are exposed immediately for report-material updates such as "临时上传合同，补充门店面积和坪效"; explicit existing-page repair prompts still hide the baseline while a background revision proceeds.

**Step 3: Add routing smoke cases**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-capability-routing-smoke.ps1 -BaseUrl https://v3.elepcloud.com -ConnectionId generic-chat-main
```

Required cases:

- report triggers above produce the Xinbai modular report with correct `focus`;
- non-report questions do not create artifacts;
- report-triggering questions still contain a normal answer body;
- temporary attachment/template references are acknowledged as answer/report material, not treated as permanent dataset changes unless explicitly ingested;
- no public tool trace leaks.

Current 2026-06-06 selected 8-server smoke:

- `static_page_xinbai_template_reference` passed, returned one artifact link with focus `取高机会`.
- `static_page_xinbai_temp_contract_area` passed, returned one artifact link with focus `经营总览`.
- `static_page_xinbai_traffic_stats` passed, returned one artifact link with focus `经营总览`.
- Ordinary guards `长期卧床老人多长时间翻身一次？`, `取高是什么意思？`, and `风险识别系统有哪些项目经历？` completed with zero artifact links.
- No third-party public URL, auth method, required request field, or existing response field changed.

**Step 4: Commit**

```powershell
git add crates scripts docs/validation
git commit -m "Stabilize platform capability routing"
git push
```

---

## Task 10: Third-Party Contract Documentation And Public Artifact Docs

**Files:**

- Update: `docs/integrations/third-party-integration-api.zh-CN.md`
- Update generated/public copy if present: `public/docs/third-party-integration-api.zh-CN.md`
- Update generated/public copy if present: `apps/web/public/docs/third-party-integration-api.zh-CN.md`
- Update if generator exists: `scripts/generate-third-party-docs.*`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Audit the current public contract**

Verify the docs explain these additive/compatible behaviors:

- `dataset_external_id` and `dataset_external_ids` are stable business group scopes;
- `available_document_external_ids` and `documentExternalId` can be combined with dataset scopes;
- if a document is already covered by a dataset group, document-level duplication is ignored;
- authorization persists for the same `conversation_external_id` across later turns;
- a changed `conversation_external_id` starts a new authorization boundary;
- uploaded template/reference files can guide report/static-page/document output without automatically becoming permanent source data;
- report card fields expose the primary page URL plus `table-data.csv`, `report.ppt`, and `report.md`;
- SSE may show progress/effect images, while the final page link is available through the fixed report/artifact fields;
- existing URLs/auth/request fields/response fields are unchanged.

Expected:

- no old V3 naming remains in current customer-facing docs; use `DataMax`;
- docs do not require already integrated third parties to rework existing calls;
- docs explain that new additive fields can be ignored by older clients.

**Step 2: Regenerate and compare public copies**

Run the existing docs generator if available. Otherwise update the Markdown source and any checked-in public copy manually.

Expected:

- source and online/public copies match for third-party scope and report-card behavior;
- generated HTML or public Markdown includes DataMax naming and current report export fields;
- no secret examples, real bearer tokens, or private customer URLs are present.

**Step 3: Commit**

```powershell
git add docs public apps/web/public scripts
git commit -m "Document current third-party DataMax contract"
git push
```

---

## Final Definition Of Done

This plan is complete only when all items are true:

- `docs/validation/datamax-main-gap-closure.md` records the final deployed commit and 8-server smoke receipts for the current head.
- Third-party public URL/auth/request/response contract is unchanged.
- Third-party ordinary 20-way, main-site 20-way, main-site streaming, static-page 5-way, report/export, data-ingestion, and document-quality smokes pass or have explicit root-cause notes.
- Xinbai report returns one clickable primary link, exposes export files, preserves normal answer text, and uses the modular monthly template by default.
- Temporary documents and dataset/document group scopes work across the same third-party conversation and do not leak across conversations.
- Historical enrichment has at least a safe summary-only dry-run receipt; real backfill remains disabled unless a reviewed tiny batch is approved.
- Authenticated model-gateway operator smoke passes with a legitimate session, or remains clearly marked pending with no bypass.
- Data-source row identity semantics are documented for collapsed tables before any production mapping change.
- Low-quality recovery remains passive and cannot block normal answers.
- Template matching excludes stale Xinbai templates from normal customer-visible reuse.
- Model-visible platform capabilities route report/static-page/document/data/proactive tasks without exposing tool traces or changing public contracts.
- Online third-party integration docs match the deployed additive contract and use DataMax naming.
- No raw credentials, raw customer rows, full documents, provider payloads, or secret env names are recorded.

## Commit Cadence

- Commit after each task that changes code or validation docs.
- Use small messages, for example:
  - `Record DataMax current-head release gate`
  - `Record summary-only enrichment audit`
  - `Stabilize Xinbai report template contract`
  - `Cover third-party scoped document chat`
  - `Record authenticated model gateway smoke`
  - `Stabilize platform capability routing`
  - `Document current third-party DataMax contract`
- Push only after local checks pass.
- Deploy to 8 server with `git pull --ff-only`; restart only services whose binaries or web build changed.
