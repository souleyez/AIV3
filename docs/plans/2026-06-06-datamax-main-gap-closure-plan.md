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

## Executable Remaining-Gap Snapshot - 2026-06-06

This section is the current single-page execution sheet. Treat the longer task sections below as implementation detail.

| Gap | Current state | Next executable action | Done when |
| --- | --- | --- | --- |
| G1: 8-server private production smoke | Read-only 8-server baseline is recorded. Local shell has no private bearer/cookie for mutating 20-way smoke. | Load private smoke credentials without committing them, then run third-party 20-way, main-site 20-way, static-page 5-way, Cloudflare fallback 2-way, report export, and document-quality smoke against `https://v3.elepcloud.com`. | `docs/validation/datamax-main-gap-closure.md` contains receipts with pass/fail, latency, service status, report links, export URLs, and no secret leakage. |
| G2: Production concurrency lock | 8 server reports chat concurrency 20, static-page/Image2 concurrency 5, Cloudflare fallback 2, Right `gpt-5.5` assistant profile concurrency 20. Platform API pool cap and starvation behavior are not fully proven by private smoke. | If G1 exposes pool starvation or timeout, adjust only internal env/model-profile/worker concurrency and document rollback. | 20 chat lanes are not starved by 5 heavy page jobs; same-conversation idempotency stays ordered. |
| G3: Static-page template operations | Xinbai accepted template, focused report links, and exports are locally/publicly validated. Production low-load prewarm is still disabled. | Enable/report prewarm only after confirming low-load guard, accepted-template reuse, and operator status visibility. Keep Xinbai monthly modular report as the default template. | Same dataset/default-prompt/focus requests reuse the accepted template first; expensive Image2/Codex work is background only unless redesign is explicit. |
| G4: Background enterprise memory rollout | Fingerprint/dedup, enrichment-run storage, worker loop, deterministic enrichment kinds, and aggregate-first local tests are implemented. 8-server migration/backfill/live smoke remain pending. | Deploy migration on 8 server, dry-run fingerprint backfill, run one-shot enrichment worker, then run duplicate read-through and aggregate Q&A smoke. | New and existing documents can enrich asynchronously; duplicate documents read through canonical chunks/facts; resume, attendance, elderly-care, and Xinbai aggregate questions use deterministic supply before retrieval. |
| G5: Low-quality answer recovery | Passive detection and fixed-scope task packaging pass locally. Hard answer gate remains disabled. Production enqueue is config-gated. | Keep hard gate disabled. Enable only passive collection first, then optionally allow `answer_quality_autofix` in the fixed allowlist after live observation. | Weak answers are collected and classified; system-defect fixes remain limited to answer/retrieval optimization files and require tests before deployment. |
| G6: Confirmed data ingestion | Local staging-plan confirmation, private staging dataset creation/reuse, guarded source sync, dedupe, same-conversation scope restore, and public-doc contract smoke passed on 2026-06-06. | Deploy to 8 server, run the live read-only source readiness smoke, then manually confirm one reviewed staging plan against a stored database source before customer-facing use. | 8-server receipt proves no write before confirmation, sync source is in-plan, confirmed rows become ordinary dataset evidence, and replies reach `dataset_ready`, `sync_started`, `sync_completed`, or truthful failure. |
| G7: Controlled streaming | Main-site create and continue SSE paths both support live answer deltas behind `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED`; third-party live answer stream remains separately controlled by `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED`. Local regressions passed on 2026-06-06. | Run main-site browser/local smoke and 8-server private streaming smoke after deployment. Keep third-party final-answer release gated while streaming progress/artifact status. | Main site can show live answer deltas for new and continued runs behind flag; third-party sees progress and links but no rejected final-answer fragments. |
| G8: Operator observability | Validation ledgers exist, but operators still need one compact page for queues, model lane, report tasks, enrichment backlog, low-quality cases, and data-ingestion staging/sync. | Expand the existing external integration/operator page with lazy-loaded health panels and sanitized counts only. | A 20+ conversation incident can be triaged from DataMax without reading raw logs or exposing secrets. |
| G9: Final release hygiene | Local branch is ahead of origin and several local-only improvements are committed. | Before deployment, run local gate, push, pull on 8 server, build changed binaries, restart only changed services, then rerun private smoke. | 8 server is on latest `main`, service statuses are active, validation docs record every command and result. |

### Current Execution Order

1. Finish G7 next because streaming is customer-visible and must not leak low-quality deltas.
2. Roll out G4 migration/backfill/enrichment on 8 server with dry-run receipts before enabling background processing broadly.
3. Run G6 live source readiness and one confirmed staging-plan smoke after deployment.
4. Run G1/G2 private production smoke after deployment and config verification.
5. Enable or defer G3 production prewarm based on the smoke result and operator capacity.
6. Enable G5 passive live collection only after ordinary Q&A and report routing remain stable.
7. Add G8 observability once the queue/status surfaces have the exact counters proven by smoke.
8. Close G9 by pushing, deploying, recording receipts, and freezing the next release baseline.

### Required Smoke Set For The Next Deployment

Run locally first when possible, then on or against 8 server:

```powershell
cargo fmt --check -p platform-api -p storage -p retrieval-worker -p codex-host-agent
cargo check -p platform-api -p retrieval-worker -p codex-host-agent
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
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-capability-routing-smoke.ps1 -BaseUrl https://v3.elepcloud.com
node scripts/smoke/external-channel-20way.mjs --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20
node scripts/smoke/main-chat-20way.mjs --base-url https://v3.elepcloud.com --concurrency 20
node scripts/smoke/static-page-5way.mjs --base-url https://v3.elepcloud.com --concurrency 5
node scripts/smoke/cloudflare-fallback-2way.mjs --base-url https://v3.elepcloud.com
bash scripts/run-data-ingestion-staging-sync-smoke.sh
git diff --check
```

If private credentials are unavailable, record that explicitly in `docs/validation/datamax-main-gap-closure.md` and run only read-only/no-token guard probes. Do not claim a production pass from no-token probes.

### Implementation Boundaries For The Next Slices

- Data ingestion confirmation may add internal/operator endpoints, events, UI actions, and smoke scripts, but must not change public third-party message fields.
- Report export support may add artifact metadata and downloadable files, but the customer-visible answer should show one primary report link only.
- Static-page prewarm must never block ordinary Q&A; it should run only when a matching accepted template does not already exist and the low-load guard passes.
- Background enrichment must be asynchronous, idempotent by document fingerprint and enrichment kind, and safe to retry.
- Passive answer-quality recovery must collect and diagnose; it must not suppress normal customer answers.
- Controlled streaming must separate progress/status from final answer release in third-party channels.

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

**Status:** in progress as of 2026-06-06. Accepted-template reuse and the Xinbai report template contract are validated locally/publicly. Prewarm task construction exists, but production low-load prewarm remains disabled on 8 server and operator observability still needs closure.

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

**Status:** in progress as of 2026-06-06. Storage schema, third-party parse fingerprint capture, main-site local register fingerprint capture, zip child-document fingerprint capture, a dry-run capable existing-document fingerprint backfill tool, canonical read-through for chunks/evidence/facts, the `document_enrichment_runs` repository foundation, feature-flagged post-ingest enrichment enqueue, document-level enrichment diagnostics, and the low-priority enrichment worker execution loop are implemented locally. 8-server migration/backfill rollout and live smoke remain pending.

**Files:**

- Modify: `crates/storage/migrations/mod.rs` or migration registry if present
- Create: `crates/storage/migrations/0013_document_canonical_enrichment.sql`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/ingest-worker/src/lib.rs`
- Create: `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`
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

**Status:** completed locally on 2026-06-06; 8-server live enrichment/backfill smoke remains pending. Background enrichment worker now supports `table_structure_v1`, `entity_terms_v1`, `procedure_steps_v1`, `resume_profile_v1`, and `spreadsheet_metrics_v1` in addition to the Phase 1 kinds. Post-ingest enrichment enqueue includes the new kinds when `DOCUMENT_ENRICHMENT_ENABLED=true`. `fact_index` now emits `procedure_step` and `time_threshold` facts for care-operation text and includes those fact types in dataset entity snapshots. Fixture coverage has been added for elderly-care procedure facts and enrichment-specific resume/table/attendance checks. Full local document-quality smoke passed on 2026-06-06.

**Files:**

- Modify: `crates/platform-api/src/fact_index.rs`
- Modify: `crates/retrieval-worker/src/main.rs`
- Modify: `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`
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

Local progress:

- Implemented aliases for the plan names and versioned worker kinds.
- Added deterministic summaries for table structures, entity terms, procedure steps, resume profiles, and spreadsheet/attendance metrics.
- Kept the worker standalone and low-priority; no model calls or VLM reparse are introduced here.

**Step 3: Improve fact ranking before the cap**

Extend `crates/platform-api/src/fact_index.rs` so it:

- filters TOC dot-leader noise;
- keeps facts from later sections;
- prefers procedure headings and concrete step lists;
- extracts company/project/role/time facts from resumes;
- extracts date/hour/attendance facts from spreadsheets;
- keeps source locators.

Local progress:

- Added `procedure_step` and `time_threshold` fact types to the dataset entity snapshot type list.
- Added conservative procedure sentence and time-threshold extraction for care manuals.
- Boosted procedure/time-threshold fact rank and confidence so late operational manual sections survive the fact cap.

**Step 4: Rebuild dataset fact snapshots after enrichment**

Ensure retrieval-worker or ingest-worker schedules snapshot refresh after enrichment completes.

Local progress:

- Existing `fact_index_v2` enrichment rebuilds document facts and refreshes the dataset entity rows snapshot.
- Duplicate alias enrichment runs skip fact writes and report canonical read-through instead of double-counting.

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

**Status:** completed locally on 2026-06-06; 8-server private aggregate smoke remains pending because private bearer/cookie config is not present in this local shell.

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

Local progress:

- Added an internal deterministic aggregate intent detector covering:
  - document/entity count, list, rank, and table questions;
  - resume company/project/skill/position/location/education/certificate/experience ranking;
  - attendance absence, work-hour, and date row questions;
  - store/brand/category/risk/opportunity/take-high/low-active business metrics.
- Kept business-metric aggregate detection separate from generic document entity scans so database-backed operating questions can use database aggregates without forcing a full document scan unless the user explicitly points at documents, files, attachments, tables, or parsed materials.
- Tightened model-facing guidance for `dataset_fact_snapshot`, `document_facts_scoped_aggregate`, `database_aggregate`, `dataset_entity_scan`, and `spreadsheet_row_analysis`:
  - cite deterministic scope and row counts;
  - use `row_count_by_type`, `scanned_document_count`, `source_document_count`, `source_fact_count`, `scan_limit`, and `result_row_count` where applicable;
  - treat retrieval chunks as examples/source wording only, not proof of full-dataset totals.
- Updated the ReAct planning catalog so models see deterministic aggregate supply actions before retrieval top-k for aggregate/statistical questions.

**Step 4: Verify with private smoke**

Run local and 8-server cases:

- 简历公司名统计;
- 14 份简历项目经验排序;
- 考勤缺勤/工时长短;
- 新百风险/取高/低活跃统计;
- 养老手册操作规范问答.

Local verification on 2026-06-06:

```powershell
cargo fmt --check -p platform-api
cargo test -p platform-api dataset_fact_snapshot --lib
cargo test -p platform-api scoped_fact --lib
cargo test -p platform-api database_aggregate_heuristics --lib
cargo test -p platform-api assistant_run_general_entity_scan_prompts_request_dataset_scan --lib
cargo test -p platform-api assistant_run_deterministic_aggregate_intent_covers_customer_smoke_domains --lib
cargo test -p platform-api planning_catalog_prefers_aggregate_fact_supply_without_fact_rows --lib
cargo test -p platform-api assistant_run_answer_quality_judge_runs_for_structured_short_answer --lib
cargo test -p platform-api assistant_run_answer_quality_judge_skips_satisfied_spreadsheet_table --lib
cargo test -p platform-api assistant_run_answer_quality_gate_retries_deferred_retrieval_language --lib
cargo check -p platform-api
powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local
```

Full local smoke receipt: `target/document-quality-smoke/document-quality-smoke-20260606T010841Z-27544.json` and `target/document-quality-smoke/document-quality-smoke-20260606T010841Z-27544.md`.

8-server private smoke remains pending until the private bearer/cookie configuration is available.

**Step 5: Commit**

```powershell
git add crates docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md docs/validation/document-understanding-smoke.md
git commit -m "Prefer deterministic facts for aggregate answers"
```

---

## Task 8: Add Low-Quality Answer Monitoring And Fixed-Scope Autofix

**Status:** completed locally on 2026-06-06; production enqueue remains configuration-gated by `CODEX_HOST_TASK_ENABLED` and `CODEX_HOST_TASK_ALLOWLIST`.

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

Local progress:

- Passive case collection remains post-answer and records `blocking_gate_enabled=false`.
- Added passive signals for:
  - deterministic aggregate supply ignored by an insufficient-evidence answer;
  - report/static-page request completed without a usable generated-artifact link;
  - repeated fallback/timeout/provider-failure events in the same run.
- Existing signals remain covered for user complaints, strong complaints, internal marker/tool-call leaks, weak insufficient answers, retry exhaustion, controlled fallback, and parse-quality degradation without VLM upgrade.

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

Local progress:

- Fixed-task allowlist remains restricted to answer-quality files, document-quality fixtures, smoke scripts, and validation docs.
- `answer_quality_autofix` output validation still rejects out-of-scope files, missing tests, missing rollback notes, invalid failure types, and high-risk patches without human review.
- Operation docs now match the actual legacy smoke script path `scripts/run-v3-quality-gate-smoke.ps1`.

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

Local verification on 2026-06-06:

```powershell
cargo fmt --check -p platform-api -p codex-host-agent
cargo test -p platform-api answer_quality_autofix --lib
cargo test -p codex-host-agent answer_quality --lib
cargo test -p platform-api assistant_run_answer_quality_gate --lib
powershell -ExecutionPolicy Bypass -File .\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary
powershell -ExecutionPolicy Bypass -File .\scripts\run-v3-quality-gate-smoke.ps1 -Local
cargo check -p platform-api -p codex-host-agent
```

Smoke receipts:

- Fixed-task smoke JSON: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T011735Z.json`.
- Fixed-task smoke Markdown: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T011735Z.md`.
- Quality-gate smoke JSON: `target/document-quality-smoke/document-quality-smoke-20260606T011549Z-23164.json`.
- Quality-gate smoke Markdown: `target/document-quality-smoke/document-quality-smoke-20260606T011549Z-23164.md`.

**Step 6: Commit**

```powershell
git add crates fixtures scripts docs/operations/answer-quality-autofix.md docs/validation
git commit -m "Add passive answer quality autofix loop"
```

---

## Task 9: Close Confirmed Data Ingestion To Dataset Sync

**Status:** completed locally on 2026-06-06; 8-server live source readiness and one confirmed staging-plan smoke remain pending after deployment.

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

Local progress:

- Internal operator routes are present:
  - `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/confirm`;
  - `POST /v1/assistant-runs/{run_id}/data-ingestion-staging-plans/{plan_id}/sync`.
- Confirmation loads only a `v3_data_ingestion_staging_plan` already attached to the AssistantRun output artifacts.
- Confirmation creates or reuses a private DataMax staging dataset and records `assistant_run.data_ingestion_staging_plan_confirmed`.
- Sync refuses unconfirmed plans, validates the selected database source is included in the plan, starts `ExternalSourceSync` into the confirmed dataset, records `assistant_run.data_ingestion_staging_sync_started`, and deduplicates repeated sync clicks by default.
- Workflow progress records sanitized `assistant_run.data_ingestion_staging_sync_updated` events; third-party reply recovery maps them to running/completed/failed states.
- Same `conversation_external_id` follow-up turns can restore a completed staging dataset as visible scope after sync.
- Third-party public request fields are unchanged; docs describe only additive statuses/cards.

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

Local verification on 2026-06-06:

```powershell
bash scripts/run-data-ingestion-staging-sync-smoke.sh
```

Passed checks:

- `cargo test -p platform-api data_ingestion --lib` passed, 13 tests.
- `cargo test -p platform-api external_source_sync --lib` passed, 6 tests.
- `cargo test -p external-source-worker` passed, 11 tests.
- `cargo test -p ingest-worker` passed.
- `cargo test -p retrieval-worker` passed.
- `DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true bash scripts/run-data-ingestion-staging-live-smoke.sh` passed through the wrapper.
- `npm run check:pure-third-party-guide-html` passed.

Smoke receipts:

- JSON: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T012507Z.json`.
- Markdown summary: `target/data-ingestion-staging-sync-smoke/data-ingestion-staging-sync-smoke-20260606T012507Z.md`.
- Live readiness self-test JSON: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T012938Z.json`.
- Live readiness self-test Markdown: `target/data-ingestion-staging-sync-smoke/live-self-test/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T012938Z.md`.

Remaining:

- deploy current code to 8 server;
- run `bash scripts/run-data-ingestion-staging-live-smoke.sh` on 8 server against the stored source;
- manually confirm one reviewed staging plan and start sync through the internal operator routes;
- record that the synced dataset contains source-derived documents, chunks, retrieval evidence, and no raw credentials/table dumps.

**Step 5: Commit**

```powershell
git add crates apps/web scripts docs/validation docs/plans/2026-05-21-database-source-integration-plan.md
git commit -m "Close confirmed data ingestion sync flow"
```

---

## Task 10: Roll Out Controlled Streaming Safely

**Status:** in progress as of 2026-06-06. Main-site `continue/stream` now uses the same live-delta worker/channel path as new AssistantRun creation when `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true`. Third-party stream behavior was not changed; existing safety/replay regressions still pass. 8-server private streaming smoke remains pending.

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

Local progress:

- `/v1/assistant-runs/stream` already emitted `assistant_run.delta` behind `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED`.
- `/v1/assistant-runs/{run_id}/continue/stream` now also uses a live worker/channel when that flag is enabled.
- Continue streaming reuses `AssistantRunLiveDeltaSink` and `complete_assistant_run_provider_live_streaming`.
- Continue completion can skip the final full-text delta if live deltas already reached the browser, avoiding duplicate text in the main-site chat UI.
- Non-streaming JSON continue and background model-completion recovery still pass `None` for the live sink.

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

Local verification on 2026-06-06:

```powershell
cargo fmt --check -p platform-api
cargo test -p platform-api assistant_run_sse --lib
cargo test -p platform-api assistant_run_continue_sse --lib
cargo test -p platform-api assistant_run_live --lib
cargo test -p platform-api assistant_run_continue --lib
cargo test -p platform-api external_channel_stream_resume --lib
cargo test -p platform-api external_channel_public_stream --lib
cargo test -p platform-api generic_chat_page_event_stream_can_emit_live_answer_delta_without_final_duplication --lib
cargo check -p platform-api
```

Notes:

- `cargo test -p platform-api assistant_run_streaming --lib` currently has no matching tests; the actual main-site filters above are the authoritative local coverage for this slice.
- `cargo test -p platform-api external_channel_streaming --lib` currently has no matching tests; third-party stream coverage is under `external_channel_public_stream`, `external_channel_stream_resume`, and the named generic-chat live delta test.

Remaining:

- run browser/local smoke for main-site new and continued AssistantRun streaming with `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true`;
- run 8-server private external streaming smoke with the active bearer;
- record whether `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED=true` remains enabled or should be kept as progress-only until the 8-server smoke passes.

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

Use `Executable Remaining-Gap Snapshot - 2026-06-06` as the current execution sheet. The detailed task bodies above remain the implementation reference.

1. Task 10: roll out controlled streaming safely.
2. Task 5 and Task 6: deploy migration/backfill/enrichment on 8 server and validate live read-through.
3. Task 9: run 8-server live source readiness and one confirmed staging-plan smoke after deployment.
4. Task 7: rerun private aggregate-first smoke on 8 server after deployment.
5. Task 2 and Task 3: run 20-way private production smoke and lock config only if smoke exposes gaps.
6. Task 4: enable or defer static-page low-load prewarm based on smoke and operator readiness.
7. Task 8: keep passive answer-quality recovery enabled only after core chat/report routes are stable; hard gate remains disabled.
8. Task 11: expand operator observability using the counters proven in the earlier tasks.
9. Task 12: final local gate, push, deploy to 8 server, private smoke, and validation commit.

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
