# DataMax Main Gap Closure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the remaining major DataMax production gaps after the model-visible capability loop rollout: 20-way concurrency, background document enrichment, deterministic enterprise facts, low-quality answer recovery, confirmed data ingestion, controlled streaming, static-page template operations, and 8-server validation.

**Architecture:** Keep DataMax as the system of record for tenants, datasets, document scopes, parsed evidence, fact snapshots, AssistantRun state, workflow tasks, generated artifacts, and third-party contracts. New work must add internal queues, validation ledgers, background enrichment, and operator views around existing APIs rather than changing public third-party URLs, auth, or request/response fields. Customer-facing answers stay model-authored; platform tools provide scoped evidence, reports, status, and artifact links.

**Tech Stack:** Rust `platform-api`, `storage`, `ingest-worker`, `retrieval-worker`, `static-page-worker`, `codex-host-agent`, PostgreSQL, NATS/workflow tasks, Next.js DataMax web app, existing smoke scripts under `scripts/`, 8-server systemd services, Cloudflare/Image2/Codex fixed-task bridge.

---

## Current Baseline - 2026-06-06

- Local `main` was clean before this plan document was added.
- 8 server was on commit `03458d310`; `aiv3-platform-api.service` was `active`.
- The model-visible capability loop is closed for product-level capabilities:
  - `static_page_artifact`;
  - `data_ingestion_analysis`;
  - `document_processing`;
  - `collection_setup_analysis`;
  - `integration_setup_analysis`;
  - `message_channel_outreach`.
- Third-party Xinbai report smoke passed for:
  - `取高`;
  - `经营状况`;
  - `经营健康度`;
  - `看看整体经营情况`;
  - `风险识别`;
  - `看看新街口店经营风险`;
  - `销售缺口统计一下，哪些门店需要助推？`;
  - ordinary-Q&A non-trigger guards such as `取高是什么意思？`.
- Xinbai default report template is `xinbai-functional-modular-template-20260604`.
- Report card enrichment restores:
  - title `新世界百货经营管理月报表`;
  - focused report URL;
  - `table-data.csv`;
  - `report.ppt`;
  - `report.md`;
  - `download_exports[]`.
- Existing detailed plans remain the source for lower-level implementation detail:
  - `docs/plans/2026-05-31-v3-20-way-concurrency-upgrade-plan.md`;
  - `docs/plans/2026-05-30-static-page-image-pipeline-optimization-plan.md`;
  - `docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md`;
  - `docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md`;
  - `docs/plans/2026-06-01-v3-streaming-session-upgrade-plan.md`;
  - `docs/plans/2026-05-21-database-source-integration-plan.md`;
  - `docs/plans/2026-06-04-v3-model-visible-capability-loop-plan.md`.

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

**Status:** in progress as of 2026-06-06. Template reuse and prewarm implementation/tests exist; 8-server prewarm env is not enabled yet.

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

**Status:** in progress as of 2026-06-06. Phase 1 storage schema for fingerprints, canonical document aliases, and enrichment runs is implemented locally; ingest-time fingerprint capture is still pending.

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

**Status:** pending

**Files:**

- Modify: `crates/storage/migrations/mod.rs` or migration registry if present
- Create: `crates/storage/migrations/0013_document_canonical_enrichment.sql`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/ingest-worker/src/lib.rs`
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

**Status:** pending

**Files:**

- Modify: `crates/platform-api/src/fact_index.rs`
- Modify: `crates/retrieval-worker/src/lib.rs`
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

**Step 3: Improve fact ranking before the cap**

Extend `crates/platform-api/src/fact_index.rs` so it:

- filters TOC dot-leader noise;
- keeps facts from later sections;
- prefers procedure headings and concrete step lists;
- extracts company/project/role/time facts from resumes;
- extracts date/hour/attendance facts from spreadsheets;
- keeps source locators.

**Step 4: Rebuild dataset fact snapshots after enrichment**

Ensure retrieval-worker or ingest-worker schedules snapshot refresh after enrichment completes.

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

**Status:** pending

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

**Step 4: Verify with private smoke**

Run local and 8-server cases:

- 简历公司名统计;
- 14 份简历项目经验排序;
- 考勤缺勤/工时长短;
- 新百风险/取高/低活跃统计;
- 养老手册操作规范问答.

**Step 5: Commit**

```powershell
git add crates docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md docs/validation/document-understanding-smoke.md
git commit -m "Prefer deterministic facts for aggregate answers"
```

---

## Task 8: Add Low-Quality Answer Monitoring And Fixed-Scope Autofix

**Status:** pending

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

**Step 3: Route only system-defect cases to fixed-task autofix**

Use existing `answer_quality_autofix` with strict write scope:

- `crates/platform-api/src/lib.rs`;
- `crates/platform-api/src/fact_index.rs`;
- `fixtures/document-quality/**`;
- `scripts/run-document-quality-smoke.ps1`;
- `scripts/run-v3-quality-gate-smoke.ps1`.

Reject:

- missing customer data;
- ambiguous user request;
- unsupported public contract change;
- required credential access.

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

**Step 6: Commit**

```powershell
git add crates fixtures scripts docs/operations/answer-quality-autofix.md docs/validation
git commit -m "Add passive answer quality autofix loop"
```

---

## Task 9: Close Confirmed Data Ingestion To Dataset Sync

**Status:** pending

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

**Step 5: Commit**

```powershell
git add crates apps/web scripts docs/validation docs/plans/2026-05-21-database-source-integration-plan.md
git commit -m "Close confirmed data ingestion sync flow"
```

---

## Task 10: Roll Out Controlled Streaming Safely

**Status:** pending

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

**Step 5: Commit**

```powershell
git add crates apps/web scripts docs/plans/2026-06-01-v3-streaming-session-upgrade-plan.md docs/integrations apps/web/public/external-integrations
git commit -m "Roll out controlled DataMax streaming"
```

---

## Task 11: Expand Operator Observability

**Status:** pending

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

1. Task 1: validation ledger.
2. Task 2: 20-way production validation.
3. Task 3: config lock only if Task 2 exposes gaps.
4. Task 4: static-page template operations and prewarm.
5. Task 10: controlled streaming, because it affects customer-visible behavior and third-party perception.
6. Task 8: passive answer-quality monitoring, with hard gate still disabled.
7. Task 5 and Task 6: background enrichment and fact extraction.
8. Task 7: aggregate-first answer supply after enrichment foundations are present.
9. Task 9: confirmed data-ingestion sync.
10. Task 11: operator observability.
11. Task 12: final 8-server release gate.

## Definition Of Done

- 8 server has recorded receipts for all P0 gates.
- Main-site and third-party ordinary chat pass 20-way smoke.
- Heavy jobs are bounded and do not starve chat.
- Xinbai report reuse returns a focused link immediately and exposes export files.
- Same-domain report requests prefer the accepted template unless redesign is explicit.
- Documents continue to enrich after ingestion without blocking upload or chat.
- Multi-document aggregate questions use deterministic fact snapshots before retrieval.
- Low-quality answer recovery is passive, audited, fixed-scope, and regression-tested.
- Data-ingestion analysis can proceed to confirmed staging sync without automatic production writes.
- Streaming surfaces are consistent across JSON, SSE, and polling.
- Operator view shows enough health data to diagnose queue, model, report, enrichment, and data-ingestion issues.
- No public third-party contract break was introduced.
