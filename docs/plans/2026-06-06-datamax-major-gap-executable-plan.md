# DataMax Major Gap Closure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the remaining major DataMax production gaps with small, reversible tasks: release-gate verification, report/static-page stability, third-party scoped-document reliability, data-source row identity, historical enrichment, authenticated model-gateway validation, passive answer-quality recovery, low-load template prewarm, and public contract documentation.

**Architecture:** DataMax remains the system of record for tenant scope, conversation authorization, document parsing, deterministic facts, workflow tasks, generated artifacts, model routing, and third-party integration contracts. This plan only adds validation, guarded operators, staging-only experiments, and passive recovery around existing flows. Do not change third-party public URLs, authentication, required request fields, or existing response fields without explicit operator approval.

**Tech Stack:** Rust workspace crates (`platform-api`, `storage`, `ingest-worker`, `retrieval-worker`, `static-page-worker`, `codex-host-agent`), PostgreSQL, Axum/SSE, workflow task queues, Next.js `apps/web`, Node smoke scripts, PowerShell/Bash smoke wrappers, 8-server systemd services, DataMax Image2/static-page template pipeline.

---

## Execution Order

1. **P0 release gate:** prove current local head and 8-server head, then run the core smoke set.
2. **P0 customer-visible report path:** keep Xinbai report/template behavior stable, one link only, exports available, normal answer not truncated.
3. **P0 scoped third-party documents:** keep temporary attachments, dataset scopes, document scopes, and same-conversation authorization reliable.
4. **P1 data-source row identity:** resolve `bi_contract_warning` and `bi_rentsales_detail` row-level semantics in staging only.
5. **P1 historical enrichment:** keep default dry-run; run one real single-document enrichment only after explicit approval.
6. **P1 authenticated operator checks:** complete model-gateway smoke with a legitimate session or local-key login, no bypass.
7. **P1 passive quality recovery:** keep customer-facing hard gate disabled; add only passive review/diagnostics unless explicitly approved.
8. **P1 low-load template prewarm:** enable only after queue/backpressure and end-to-end prewarm smoke prove it is safe.
9. **P2 observability and docs:** keep operator status and public third-party docs aligned with additive behavior.

## Non-Negotiable Guardrails

- Do not touch 120 server.
- Use 8 server as the production/demo validation target.
- Do not change third-party public URLs, auth, required request fields, existing response fields, or public status values without asking first.
- Additive response fields are allowed only when old clients can safely ignore them.
- Do not enable a customer-facing hard answer-quality gate.
- Do not run real historical backfill/enrichment without explicit operator approval and a tiny reviewed scope.
- Do not print raw credentials, bearer tokens, cookies, local keys, database URLs, provider payloads, raw customer rows, full customer documents, or local object paths in docs or smoke receipts.
- Treat VLM reparse as premium and budget-gated. Use deterministic parsing/enrichment first.
- If deleting local generated artifacts is requested later, follow the local backup-first policy for unclear/source-like files.

## Current Known Open Gaps

| Gap | Current State | Target |
| --- | --- | --- |
| Authenticated model-gateway smoke | Unauthenticated guard returns `401 auth_session_required`; no legitimate operator credential was available. | Run with a real operator cookie or email/local-key and record sanitized receipt. |
| Data-source row identity | Latest audit shows 193 collapsed latest-sync rows: `bi_contract_warning` 94, `bi_rentsales_detail` 99. | Decide entity/latest-snapshot vs row-level detail; if row-level, prove staging discriminator before production mapping changes. |
| Historical enrichment | Guarded dry-runs work; reviewed dataset has `file_not_found=20`; one reachable single-document candidate exists. | Keep disabled by default; optionally approve one single-document real enqueue and monitor. |
| Passive low-quality recovery | Hard gate is disabled; live autofix cannot run under current flags/allowlists. | Add passive collection/manual review only, without blocking answers. |
| Low-load static-page prewarm | Accepted-template reuse works; production prewarm flag is not active. | Enable only after prewarm queue, load, and artifact smoke prove it is safe. |
| Release gate drift | Core flows have receipts, but future code changes need the same gate repeated. | Keep the command set below as the minimum before GitHub + 8-server deployment closure. |

---

## Task 1: Current-Head Release Gate

**Files:**

- Update: `docs/validation/datamax-main-gap-closure.md`
- Update if failure reveals a bug: `scripts/smoke/external-channel-20way.mjs`
- Update if failure reveals a bug: `scripts/smoke/main-chat-20way.mjs`
- Update if failure reveals a bug: `scripts/smoke/main-assistant-streaming.mjs`
- Update if failure reveals a bug: `scripts/smoke/static-page-5way.mjs`
- Update if failure reveals a bug: `scripts/smoke/external-report-export.mjs`
- Update if failure reveals a bug: `scripts/smoke/external-scoped-document-chat.mjs`
- Update if failure reveals a bug: `scripts/run-document-quality-smoke.ps1`
- Update if failure reveals a bug: `scripts/run-data-ingestion-staging-live-smoke.sh`

**Step 1: Verify local status**

Run:

```powershell
git status --short --branch
git rev-parse --short HEAD
```

Expected:

- branch is `main`;
- no local uncommitted source/doc changes before starting;
- if there are changes, list them and decide whether they belong to this pass.

**Step 2: Verify 8-server status**

Run:

```powershell
ssh 8服务器 "cd /srv/aiv3/repo && git status --short --branch && git rev-parse --short HEAD && systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-codex-host-agent.service aiv3-document-enrichment-worker.service aiv3-ingest-worker.service aiv3-retrieval-worker.service aiv3-static-page-worker.service"
```

Expected:

- repo is on `main`;
- known untracked `mode` may exist and must be left untouched;
- listed services are `active`.

**Step 3: Run local non-secret checks**

Run:

```powershell
cargo fmt --check
cargo check -p platform-api
cargo check -p retrieval-worker --bin document-enrichment-backfill
node --check scripts\smoke\external-report-export.mjs
node --check scripts\smoke\external-scoped-document-chat.mjs
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local
```

Expected:

- all commands pass;
- document-quality smoke still covers one-character PDF, `邓工是谁`, elderly-care procedures, resume aggregation, attendance table, smart-home, and smart-elevator fixtures.

**Step 4: Run 8-server public/private smokes**

Use existing private bearer/session sources without printing them. Required smokes:

```bash
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20 --timeout-ms 120000
npm run smoke:main-chat-20way -- --base-url https://v3.elepcloud.com --dataset-id <main-site-visible-dataset-uuid> --concurrency 20 --timeout-ms 90000 --poll-timeout-ms 120000
npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com --timeout-ms 120000 --require-live-delta --require-multiple-deltas
npm run smoke:static-page-5way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --timeout-ms 240000
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --timeout-ms 180000
npm run smoke:external-scoped-document-chat -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --source-id third-party-source-main --timeout-ms 180000 --parse-timeout-ms 240000
DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY=hy-sql-traffic-area DATA_INGESTION_LIVE_SMOKE_API_BASE=http://127.0.0.1:3000 bash scripts/run-data-ingestion-staging-live-smoke.sh
```

Expected:

- third-party ordinary chat passes 20-way;
- main-site chat passes 20-way;
- main-site live streaming emits real deltas and one completion;
- static-page 5-way returns artifacts;
- report export smoke sees one report link and accessible `table-data.csv`, `report.ppt`, `report.md`;
- scoped-document smoke proves dataset scope, document scope, same-conversation reuse, and changed-conversation isolation;
- data-ingestion smoke remains ready or clearly reports row-identity attention without raw rows.

**Step 5: Record receipts**

Append to `docs/validation/datamax-main-gap-closure.md`:

- local head;
- 8-server head;
- service status;
- smoke command shapes;
- receipt paths;
- pass/fail counts;
- any pending credential-only gates.

**Step 6: Commit if docs changed**

Run:

```powershell
git add docs/validation/datamax-main-gap-closure.md
git commit -m "Record DataMax release gate receipts"
git push
```

---

## Task 2: Xinbai Report And Static-Page Contract

**Files:**

- Inspect/modify if behavior is wrong: `crates/platform-api/src/lib.rs`
- Inspect/modify if template matching is wrong: `crates/storage/src/lib.rs`
- Inspect/modify if host output is wrong: `crates/codex-host-agent/src/main.rs`
- Inspect/modify if runtime rendering is wrong: `crates/static-page-runtime/src/lib.rs`
- Inspect/modify: `scripts/smoke/external-report-export.mjs`
- Inspect/modify: `scripts/run-external-capability-routing-smoke.ps1`
- Validate: `docs/static-page-templates/xinbai-functional-modular-template-20260604/README.md`
- Update: `docs/validation/external-report-export-smoke.md`
- Update: `docs/validation/static-page-render-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Write or update focused tests before changing behavior**

Run the existing focused tests first:

```powershell
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api external_channel_public_response --lib
cargo test -p platform-api static_page_template --lib
cargo test -p platform-api external_channel_static_page_dataset_template_overlap --lib
cargo test -p platform-api external_channel_public_response_enriches_xinbai_report_card_exports --lib
npm run validate:xinbai-report-template
```

Expected:

- report response exposes one primary customer-facing link;
- `artifact_links`/card fields contain the same primary page URL;
- exports are available through machine-readable fields;
- focus is present when the request implies focus.

**Step 2: Keep customer response shape stable**

If a bug is found, modify `crates/platform-api/src/lib.rs` so that:

- normal answer text is not truncated when a report/static-page task is triggered;
- customer-visible text contains only one primary report link;
- report URL is also present in structured card/artifact fields;
- `table-data.csv`, `report.ppt`, and `report.md` remain sibling export files;
- public static-page cards say the report was generated according to the customer requirement, not that an old template was reused.

Expected:

- existing response fields remain unchanged;
- any new fields are additive and ignorable.

**Step 3: Keep Xinbai modular monthly report as default**

Validate that normal Xinbai report matching prefers:

- `xinbai-functional-modular-template-20260604`;
- dataset overlap;
- same or close `default_prompt`;
- focus-based module ordering instead of creating a new design by default.

Do not delete old generated artifacts in this task.

**Step 4: Run live 8-server report smoke**

Run with the active private bearer loaded without printing:

```bash
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --dataset-external-ids 64fff6c8-10e2-4ee8-8243-23166cce3abc --timeout-ms 180000 --expected-title 新世界百货经营管理月报表 --expected-focus 取高机会
```

Expected:

- JSON and SSE both pass;
- one clickable primary report link;
- focus is `取高机会`;
- export files return HTTP 200;
- no duplicate URL text.

**Step 5: Add misroute fixtures when customer wording fails**

Update `scripts/run-external-capability-routing-smoke.ps1` only when needed. Must cover:

- report triggers: `取高`, `经营状况`, `经营健康度`, `整体经营情况`, `风险识别`, `销售缺口`, `助推`, `哪些门店需要关注`, `坪效`, `客流统计`;
- non-report guards: `取高是什么意思？`, `风险识别系统有哪些项目经历？`;
- temporary report material: uploaded template/contract/traffic-stat references.

Run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-capability-routing-smoke.ps1 -BaseUrl https://v3.elepcloud.com -ConnectionId generic-chat-main -DatasetExternalIds @("64fff6c8-10e2-4ee8-8243-23166cce3abc")
```

Expected:

- report triggers return an artifact link with correct focus;
- non-report guards return zero artifact links;
- ordinary answer still appears.

**Step 6: Commit**

Run:

```powershell
git add crates scripts docs/validation docs/static-page-templates
git commit -m "Stabilize Xinbai report contract"
git push
```

---

## Task 3: Third-Party Scoped Documents And Temporary Attachments

**Files:**

- Inspect/modify if scope restoration fails: `crates/platform-api/src/lib.rs`
- Inspect/modify if storage lookup fails: `crates/storage/src/lib.rs`
- Inspect/modify smoke: `scripts/smoke/external-scoped-document-chat.mjs`
- Update: `docs/validation/document-understanding-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Run scoped-document smoke**

Run:

```bash
npm run smoke:external-scoped-document-chat -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --source-id third-party-source-main --timeout-ms 180000 --parse-timeout-ms 240000
```

Expected:

- `dataset_external_ids` grants the whole dataset group for the same `conversation_external_id`;
- `available_document_external_ids` grants individual documents for the same `conversation_external_id`;
- dataset and explicit document scopes can be combined;
- documents already covered by dataset scope are deduplicated internally;
- changed `conversation_external_id` cannot see prior scope;
- temporary attachments can answer the current turn before an ACL snapshot exists.

**Step 2: Fix only if the smoke fails**

If failure appears, update the smallest matching path in `crates/platform-api/src/lib.rs`:

- same-conversation scope restore;
- dataset/document union;
- attachment-title scope supply;
- external document parse completion polling;
- response shaping for temporary document answers.

Do not change public request field names.

**Step 3: Add a regression fixture for the failing customer case**

Extend `scripts/smoke/external-scoped-document-chat.mjs` with the smallest fixture:

- source id;
- dataset external id;
- document external id;
- conversation id reuse case;
- expected answer substring or evidence condition.

**Step 4: Verify and commit**

Run:

```powershell
node --check scripts\smoke\external-scoped-document-chat.mjs
cargo test -p platform-api external_document_parse --lib
cargo test -p platform-api external_channel_scoped_document --lib
```

Then:

```powershell
git add crates scripts docs/validation
git commit -m "Cover third-party scoped document access"
git push
```

---

## Task 4: Data-Source Row Identity Decision And Staging Proof

**Files:**

- Inspect/modify staging mapping only if approved: `crates/external-source-connectors/src/mysql.rs`
- Inspect/modify sync materialization: `crates/ingest-worker/src/main.rs`
- Inspect/modify report/readiness logic only if needed: `crates/platform-api/src/lib.rs`
- Inspect/modify audit script: `scripts/run-data-ingestion-staging-live-smoke.sh`
- Update: `docs/operations/data-source-row-identity-decision.md`
- Update: `docs/validation/data-ingestion-staging-sync-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Re-run read-only identity audit**

Run on 8 server using platform env without printing database URL:

```bash
DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY=hy-sql-traffic-area DATA_INGESTION_LIVE_SMOKE_API_BASE=http://127.0.0.1:3000 bash scripts/run-data-ingestion-staging-live-smoke.sh
```

Expected:

- no customer/source database query is issued by the smoke;
- report includes table name, configured identity columns, source row count, unique document count, collapsed duplicate row count, current document count, and recommended action;
- raw rows are not printed.

**Step 2: Make the business decision explicit**

Update `docs/operations/data-source-row-identity-decision.md` with one of:

- `entity_latest_snapshot`: current collapse semantics are intended for the table;
- `row_level_required`: row-level materialization is required for reports.

Expected:

- if `entity_latest_snapshot`, report wording must not claim row-level completeness for that table;
- if `row_level_required`, production mapping still remains unchanged until staging proof passes.

**Step 3: If row-level is required, add staging-only discriminator**

Do not change production mapping first. Add or configure a staging-only discriminator for:

- `bi_contract_warning`: keep `parentcode`, `storecode`, `txdate`; add stable row discriminator fields that distinguish brand/store/contract/metric rows under the same key.
- `bi_rentsales_detail`: keep `storecode`, `contract_no`, `contract_startdate`; add stable row discriminator fields that distinguish month/date/rent-type/detail rows under the same key.

Expected:

- discriminator is deterministic;
- no raw source-row values are logged;
- staging sync shows source rows close to unique materialized documents for the affected tables.

**Step 4: Verify staging behavior**

Run:

```powershell
cargo fmt --check -p ingest-worker -p platform-api
cargo test -p ingest-worker --bin ingest-worker external_source_ingest_table_counts -- --nocapture
cargo check -p ingest-worker -p platform-api
wsl.exe --cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 env DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true bash scripts/run-data-ingestion-staging-live-smoke.sh
```

Then run the read-only 8-server live smoke again.

Expected:

- source row count remains separate from unique document/evidence count;
- collapsed rows either remain documented as intended or drop in staging after discriminator mapping;
- no production mapping is changed without approval.

**Step 5: Commit**

Run:

```powershell
git add crates scripts docs/operations docs/validation
git commit -m "Record data source row identity decision"
git push
```

---

## Task 5: Historical Enrichment And Dedup Backfill

**Files:**

- Inspect/modify guard binaries only if needed: `crates/platform-api/src/bin/document-fingerprint-backfill.rs`
- Inspect/modify guard binaries only if needed: `crates/platform-api/src/bin/fact-index-backfill.rs`
- Inspect/modify guard binaries only if needed: `crates/retrieval-worker/src/bin/document-enrichment-backfill.rs`
- Inspect/modify worker only if needed: `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`
- Update: `docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Re-run summary-only dry-run for the reviewed blocked dataset**

Run on 8 server:

```bash
./target/release/document-fingerprint-backfill --dataset-id cd024465-358e-458c-961d-a8894f2358c5 --limit 20 --dry-run --summary-only --pretty
./target/release/fact-index-backfill --dataset-id cd024465-358e-458c-961d-a8894f2358c5 --limit 5 --dry-run --summary-only --pretty
./target/release/document-enrichment-backfill --dataset-id cd024465-358e-458c-961d-a8894f2358c5 --kind procedure_steps,table_structure --limit 10 --dry-run --summary-only --pretty
```

Expected:

- no records written;
- summary-only output omits titles, paths, URLs, and document bodies;
- if `file_not_found` remains, do not enqueue enrichment for this dataset.

**Step 2: Re-run reachable single-document precheck**

Run:

```bash
./target/release/document-enrichment-backfill --dataset-id 1bcf2529-0bbb-46e6-884f-c2b33db352c2 --document-id 00fc651b-99f7-444b-9ee7-59695b2736cf --kind procedure_steps,table_structure --dry-run --summary-only --pretty
```

Expected:

- `missing_fingerprint_count=0`;
- `would_enqueue_count=2`;
- `enqueued_count=0`.

**Step 3: Ask before real enqueue**

Stop here unless the operator explicitly approves a real historical enrichment run.

Approval text must name:

- dataset id;
- document id;
- exact kinds;
- expected max enqueue count;
- rollback/disable step.

**Step 4: If approved, run one single-document real enqueue**

Run:

```bash
./target/release/document-enrichment-backfill --dataset-id 1bcf2529-0bbb-46e6-884f-c2b33db352c2 --document-id 00fc651b-99f7-444b-9ee7-59695b2736cf --kind procedure_steps,table_structure --confirm-real-run --summary-only --pretty
```

Expected:

- only two enrichment runs are enqueued;
- no dataset-wide enqueue occurs;
- output remains summary-only.

**Step 5: Monitor worker and results**

Run:

```bash
systemctl status aiv3-document-enrichment-worker.service --no-pager
journalctl -u aiv3-document-enrichment-worker.service -n 120 --no-pager
```

Expected:

- worker remains active;
- the two runs finish or fail with sanitized error summaries;
- no queue backlog appears.

**Step 6: Record and commit**

Update validation docs with command shapes, counts, and safety notes. Then:

```powershell
git add docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md docs/validation/datamax-main-gap-closure.md
git commit -m "Record historical enrichment precheck"
git push
```

---

## Task 6: Authenticated Model-Gateway Operator Smoke

**Files:**

- Inspect/modify only if smoke reveals a bug: `scripts/smoke/model-gateway-operator.mjs`
- Inspect/modify only if smoke reveals a bug: `crates/platform-api/src/lib.rs`
- Inspect/modify only if smoke reveals a bug: `apps/web/app/lib/model-gateway.js`
- Update: `docs/operations/model-gateway-rollout.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Confirm unauthenticated guard**

Run:

```powershell
npm run smoke:model-gateway-operator -- --base-url https://v3.elepcloud.com --allow-missing-credentials --output-dir target/model-gateway-operator-smoke-no-credentials
```

Expected:

- unauthenticated status returns `401 auth_session_required`;
- missing credential is recorded as pending, not pass.

**Step 2: Run with a legitimate operator credential**

Use exactly one approved path:

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

- profile/status calls pass;
- Right `gpt-5.5` assistant-chat profile is enabled;
- concurrency target is visible as 20;
- MiniMax fallback remains bounded;
- no cookie, local key, provider key, raw auth env, or provider payload is printed.

**Step 3: Optional provider probe only after approval**

Run only if provider-call validation is explicitly approved:

```powershell
npm run smoke:model-gateway-operator -- --base-url https://v3.elepcloud.com --cookie "<existing aidp_v3_session cookie>" --run-profile-test
```

Expected:

- sanitized provider test succeeds or fails truthfully;
- no secret is logged.

**Step 4: Commit docs**

```powershell
git add docs/operations/model-gateway-rollout.md docs/validation/datamax-main-gap-closure.md
git commit -m "Record authenticated model gateway smoke"
git push
```

---

## Task 7: Passive Low-Quality Recovery Without Blocking Answers

**Files:**

- Inspect/modify if enabling passive collection: `crates/platform-api/src/lib.rs`
- Inspect/modify if enabling passive collection: `crates/codex-host-agent/src/lib.rs`
- Inspect/modify fixtures: `fixtures/document-quality/smoke-cases.json`
- Inspect/modify smoke: `scripts/run-document-quality-smoke.ps1`
- Update: `docs/operations/answer-quality-autofix.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Reconfirm safe-disabled production state**

Run a sanitized 8-server audit that records booleans only:

- `ASSISTANT_RUN_ANSWER_QUALITY_GATE_ENABLED` absent or false;
- `ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED` absent or false;
- `CODEX_HOST_TASK_ALLOWLIST` does not include `answer_quality_autofix`;
- `CODEX_HOST_CAPABILITY_ALLOWLIST` or profile capabilities do not enable `answer_quality_autofix`.

Expected:

- normal answers cannot be blocked by the old hard gate;
- live autofix cannot enqueue accidentally.

**Step 2: Keep passive collection only**

If implementation is needed, add only passive signals for:

- insufficient-evidence language despite available deterministic supply;
- missing report/static-page artifact link when report task was triggered;
- repeated fallback/timeout/provider-failure events;
- user dissatisfaction expressions.

Expected:

- customer still receives the original answer;
- no automatic code patch is created;
- review queue or diagnostics are internal-only.

**Step 3: Add local regressions**

Run:

```powershell
cargo test -p platform-api answer_quality --lib
cargo test -p platform-api answer_quality_autofix --lib
cargo test -p platform-api assistant_run_answer_quality_gate --lib
cargo test -p codex-host-agent answer_quality --lib
powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local
```

Expected:

- hard-gate retry logic remains tested but disabled in production;
- passive signals are collected without suppressing answers.

**Step 4: Commit**

```powershell
git add crates fixtures scripts docs/operations docs/validation
git commit -m "Keep answer quality recovery passive"
git push
```

---

## Task 8: Low-Load Static-Page Template Prewarm

**Files:**

- Inspect/modify: `crates/platform-api/src/lib.rs`
- Inspect/modify: `crates/static-page-worker/src/main.rs`
- Inspect/modify: `crates/codex-host-agent/src/main.rs`
- Inspect/modify smoke if adding one: `scripts/smoke/static-page-5way.mjs`
- Update: `docs/plans/2026-05-30-static-page-image-pipeline-optimization-plan.md`
- Update: `docs/validation/static-page-render-smoke.md`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Current status 2026-06-06:** Implemented and safe-disabled. Platform code creates a customer-invisible draft and delayed `static_page/generate_static_page_image` task only when `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=true`; the static-page worker rechecks active heavy static-page/Codex work before image generation and requeues non-consumingly when load is above threshold. 8-server private smoke has already proven the local publish path can create an accepted generated-artifact template without customer-visible SSE/artifact events, and duplicate same-scope/default-prompt requests do not enqueue another prewarm task. The production default remains off; only a reviewed temporary smoke window may set `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=true`.

**Step 1: Verify prewarm remains off by default**

Run an 8-server env/status audit that records only:

- whether `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED` is true;
- current static-page queue counts;
- static-page worker active state.

Expected:

- production prewarm is off unless explicitly enabled;
- accepted-template reuse still works for explicit report/static-page requests.

**Step 2: Define low-load condition**

Implement or document the exact low-load condition before enabling:

- no current high-priority chat backlog;
- static-page/Image2 running count below configured limit;
- Codex-host fallback running count below configured limit;
- dataset combination has no accepted template for the same/default-prompt group;
- at least one dataset in the combination has no matching template.

Expected:

- prewarm never blocks or delays customer-visible chat;
- if any condition is uncertain, skip prewarm.

Current implementation detail:

- enqueue gate is controlled by `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED`;
- task start is delayed by `STATIC_PAGE_TEMPLATE_PREWARM_DELAY_MINUTES`, default 30;
- worker counts active heavy static-page and Codex-host tasks for the same tenant before running Image2;
- if active heavy task count is greater than `STATIC_PAGE_TEMPLATE_PREWARM_MAX_ACTIVE_TASKS`, default 0, the task is requeued after `STATIC_PAGE_TEMPLATE_PREWARM_RECHECK_DELAY_SECONDS`, default 300;
- customer-visible report/static-page requests, data-ingestion requests, existing accepted template matches, dataset-overlap matches, and pending same `prewarm_key` all skip prewarm creation.

**Step 3: Add plan-only smoke**

Add or run a smoke that does not create customer-visible output:

```powershell
cargo test -p platform-api static_page_template_prewarm --lib
cargo test -p platform-api static_page_template_match_tokens_allow_dataset_overlap --lib
npm run validate:xinbai-report-template
```

Expected:

- prewarm candidate is recorded as internal/silent;
- no customer message is sent;
- same dataset/default-prompt combination reuses an accepted template.

**Step 4: Enable for one controlled window only after approval**

If approved, enable `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=true` on 8 server for a controlled window.

Expected:

- one template task is queued under low load;
- artifact completes;
- later explicit report request can reuse it;
- disabling the env flag stops future prewarm.

2026-06-06 controlled smoke result: passed in a private window, then cleared. Production should stay unset/off unless a new reviewed low-load window is requested.

**Step 5: Commit docs and code**

```powershell
git add crates scripts docs/plans docs/validation
git commit -m "Prepare low-load template prewarm"
git push
```

---

## Task 9: Operator Observability

**Files:**

- Inspect/modify: `crates/platform-api/src/lib.rs`
- Inspect/modify: `apps/web/app/external-integrations/**`
- Inspect/modify: `apps/web/app/lib/model-gateway.js`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Current status 2026-06-06:** Implemented and deployed. `/external-integrations` SSR-renders the DataMax operations summary, sanitized queue stats are reachable, and unauthenticated model-gateway status still returns `401 auth_session_required`. Local and 8-server helper tests pass. The only remaining Task 9 boundary is the authenticated model-gateway operator smoke, which still requires a legitimate operator cookie or approved local-key login; do not add an auth bypass to complete it.

**Step 1: Check protected operations surfaces**

Run:

```powershell
ssh 8服务器 "curl -sS -o /tmp/external-integrations.html -w '%{http_code}' https://v3.elepcloud.com/external-integrations"
ssh 8服务器 "curl -sS -o /tmp/queue-stats.json -w '%{http_code}' http://127.0.0.1:3000/v1/workflow-tasks/queue-stats"
```

Expected:

- external integrations page returns `200`;
- queue stats returns `200` only on the protected/internal path;
- model-gateway status remains protected without operator session.

2026-06-06 check at `fcd2401`: public `/external-integrations` returned `200` and included `DataMax` plus `运营总览`; internal `/v1/workflow-tasks/queue-stats` returned `200`; unauthenticated `/v1/model-gateway/status` returned `401 auth_session_required`.

**Step 2: Ensure observability covers active gaps**

Operations summary should show sanitized status for:

- ordinary chat concurrency;
- model lane/profile health;
- workflow backlog;
- report/static-page tasks;
- template/artifact state;
- data-ingestion source readiness and row-identity attention;
- document enrichment queue and recent failures;
- answer-quality passive recovery disabled/enabled state.

Expected:

- no raw task payloads, prompts, credentials, provider payloads, or full document content;
- task details remain lazy-loaded/protected.

**Step 3: Verify web build**

Run:

```powershell
npm --prefix apps/web run build
```

Expected:

- build passes;
- `/external-integrations` still SSR-renders DataMax operations summary.

2026-06-06 check: `npm --prefix apps/web run build` passed. Existing warnings remain the Next middleware/proxy deprecation and Turbopack NFT trace warning; neither blocks the operator page.

**Step 4: Commit**

```powershell
git add apps/web crates docs/validation
git commit -m "Update DataMax operator observability"
git push
```

---

## Task 10: Third-Party Contract Docs

**Files:**

- Update: `docs/integrations/third-party-integration-api.zh-CN.md`
- Update public/generated copy if present: `public/docs/third-party-integration-api.zh-CN.md`
- Update public/generated copy if present: `apps/web/public/docs/third-party-integration-api.zh-CN.md`
- Update generator if present: `tools/render-pure-third-party-guide-html.mjs`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Audit public contract wording**

Docs must explain:

- `dataset_external_id` and `dataset_external_ids` are stable business group scopes;
- `available_document_external_ids` and `documentExternalId` can combine with dataset scopes;
- if a document is already covered by a dataset group, duplication is ignored internally;
- authorization persists for the same `conversation_external_id`;
- changing `conversation_external_id` starts a new authorization boundary;
- uploaded templates/reference files can guide report/static-page/document output without automatic permanent ingestion;
- report cards expose primary page URL plus `table-data.csv`, `report.ppt`, and `report.md`;
- SSE may show progress/effect images, while final page links are available through fixed report/artifact fields;
- existing URLs/auth/request fields/response fields are unchanged.

Expected:

- customer-facing docs use `DataMax`;
- legacy compatibility protocol names remain documented when clients still use them;
- no new mandatory field is introduced.

**Step 2: Build and compare docs**

Run:

```powershell
npm run build:pure-third-party-guide-html
npm run check:pure-third-party-guide-html
npm run test:pure-third-party-guide-html
rg -n "sk-|Bearer [A-Za-z0-9]|PLATFORM_DATABASE_URL|provider payload|V3 生成文档卡片|V3" docs public apps/web/public
```

Expected:

- docs generator/checks pass;
- no real secret examples;
- no stale user-facing `V3` branding except documented compatibility field names.

**Step 3: Verify online docs after deployment**

Run:

```powershell
curl.exe -fsSL https://v3.elepcloud.com/external-integrations/third-party-integration-api.zh-CN.html -o target\third-party-doc-full.html
curl.exe -fsSL https://v3.elepcloud.com/external-integrations/pure-third-party-integration-guide.zh-CN.html -o target\third-party-doc-simple.html
```

Expected:

- both return HTTP 200;
- online copy contains current DataMax wording and report artifact/export fields.

**Step 4: Commit**

```powershell
git add docs public apps/web/public tools
git commit -m "Document current DataMax third-party contract"
git push
```

---

## Final Definition Of Done

This plan is complete only when all items are true:

- Current local head, GitHub `main`, and 8-server `/srv/aiv3/repo` are synchronized, or intentional drift is documented.
- 8-server release-gate smokes pass: third-party 20-way, main-site 20-way, main-site streaming, static-page 5-way, report/export, scoped-document chat, document-quality, data-ingestion live smoke.
- Xinbai report returns one clickable primary link, structured artifact/card fields, and accessible `table-data.csv`, `report.ppt`, `report.md`.
- Report/static-page task triggering does not truncate ordinary answers.
- Xinbai normal matching uses the modular monthly template by default and excludes stale fallback/prewarm/smoke templates from customer-visible reuse.
- Third-party temporary documents and dataset/document scopes work across the same conversation and do not leak across changed conversations.
- `bi_contract_warning` and `bi_rentsales_detail` have a recorded business decision; any row-level production mapping change has staging proof first.
- Historical enrichment remains dry-run only, or exactly one explicitly approved single-document real enqueue is recorded with worker monitoring and rollback notes.
- Authenticated model-gateway smoke passes with a legitimate credential, or remains explicitly pending with no bypass.
- Low-quality recovery is passive and cannot suppress normal answers.
- Low-load template prewarm is off by default, or enabled only after a controlled low-load smoke proves no customer impact.
- Operator observability shows sanitized status for the active gaps.
- Public third-party docs match the deployed additive contract and use DataMax naming.
- No raw credentials, raw customer rows, full documents, provider payloads, object paths, or tokens are recorded.

## Commit Cadence

- Commit after each task that changes code, smoke scripts, or validation docs.
- Prefer small messages:
  - `Record DataMax release gate receipts`
  - `Stabilize Xinbai report contract`
  - `Cover third-party scoped document access`
  - `Record data source row identity decision`
  - `Record historical enrichment precheck`
  - `Record authenticated model gateway smoke`
  - `Keep answer quality recovery passive`
  - `Prepare low-load template prewarm`
  - `Update DataMax operator observability`
  - `Document current DataMax third-party contract`
- Push only after checks pass.
- Deploy to 8 server with `git pull --ff-only`; rebuild/restart only services whose binaries or web build changed.
