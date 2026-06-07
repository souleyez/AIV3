# DataMax Active Execution Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep one canonical executable plan for DataMax and finish the remaining work that can be handled without external credentials, production-risk approvals, or business decisions.

**Architecture:** DataMax remains the system of record for permissions, document/data ingestion, enterprise memory, model routing, static-page/report artifacts, and third-party contract behavior. This plan prioritizes local or read-only work that can be independently implemented and verified; items requiring operator credentials, business approval, provider quota, or production mutation are explicitly parked.

**Tech Stack:** Rust workspace (`platform-api`, `storage`, workers, `codex-host-agent`), PostgreSQL, workflow tasks, Next.js web app, generated third-party docs, local/8-server smoke scripts, Markdown validation ledgers.

---

## Single-Plan Policy

- `docs/plans/datamax-active-execution-plan.md` is the only active plan file.
- Historical plan files were consolidated into this file and should stay archived outside the repo backup area.
- New work should update this file instead of creating another dated plan.
- Validation evidence remains in `docs/validation/**`; operational decisions remain in `docs/operations/**`.
- Do not change third-party public URLs, auth, required request fields, or existing response fields without explicit approval.
- Do not touch 120 server from this plan.
- Do not run real historical backfill, production row-identity remapping, or production prewarm enablement without explicit operator approval.

## Current Baseline

- Local/GitHub/8-server head before this plan: `303ba10`.
- 8-server services checked active: `aiv3-platform-api.service`, `aiv3-web.service`, `aiv3-codex-host-agent.service`, `aiv3-document-enrichment-worker.service`, `aiv3-ingest-worker.service`, `aiv3-retrieval-worker.service`, `aiv3-static-page-worker.service`.
- Known 8-server repo drift: untracked `mode`; leave untouched.
- Current completion audit: `docs/validation/datamax-main-gap-closure-completion-audit.md`.
- Historical plan cleanup receipt: `C:\Users\soulzyn\Desktop\codex-backups\datamax-plan-consolidation-20260607-093057.zip`.

## External Decisions Parked

These are not blockers for the independent work queue below.

| Parked item | Why parked | Resume condition |
| --- | --- | --- |
| Authenticated model-gateway smoke | Needs legitimate operator cookie or approved local-key login | Operator provides credential; then run `npm run smoke:model-gateway-operator` |
| `bi_contract_warning` / `bi_rentsales_detail` row-level production mapping | Needs business choice: entity/latest snapshot vs row-level detail | Business says row-level detail is required; then run staging-only discriminator validation first |
| Real historical enrichment/backfill | Writes historical records or enqueues real work | Operator approves one tiny reviewed single-document run and rollback/queue monitoring |
| Production low-load template prewarm | Creates background Image2/static-page work | Operator approves a short low-load window and cleanup check |
| Full third-party database registration/sync public API | New public API/auth contract | Product decision to expose it and explicit interface review |
| Cloudflare Codex production fallback reliance | Depends on external account/quota/config | Provider/host readiness and paid/quota state confirmed |

---

## Execution Order

1. Plan hygiene and documentation cleanup.
2. Current-head baseline receipt.
3. Background enrichment/dedup diagnostics.
4. Duplicate/canonical read-through smoke.
5. Passive answer-quality offline recovery corpus.
6. Data-source row-identity staging self-test.
7. Static-page/report regression corpus tightening.
8. Third-party database read-only status hardening.
9. Operator observability polish.
10. Final validation ledger and optional deploy request.

Each task should commit independently after tests pass.

---

## Task 1: Plan Hygiene And Historical Plan Cleanup

**Files:**

- Keep: `docs/plans/datamax-active-execution-plan.md`
- Remove with backup: all other `docs/plans/*.md`
- Update if needed: `docs/validation/datamax-main-gap-closure-completion-audit.md`

**Current status 2026-06-07:** Completed. Thirty-one historical plan files were archived with `Safe-RemoveToBackup.ps1` into `C:\Users\soulzyn\Desktop\codex-backups\datamax-plan-consolidation-20260607-093057.zip`; only this active plan remains in `docs/plans`.

**Step 1: Verify this plan is the only active plan candidate**

Run:

```powershell
Get-ChildItem docs\plans -File -Filter *.md | Sort-Object Name | Select-Object Name
```

Expected before cleanup: this file plus historical plan files.

**Step 2: Move old plan files to backup**

Use the backup-first helper, never `Remove-Item`:

```powershell
$oldPlans = Get-ChildItem docs\plans -File -Filter *.md |
  Where-Object { $_.Name -ne 'datamax-active-execution-plan.md' } |
  Select-Object -ExpandProperty FullName
& "$HOME\.codex\bin\Safe-RemoveToBackup.ps1" `
  -Path $oldPlans `
  -Label "datamax-plan-consolidation" `
  -Reason "Consolidate historical DataMax plans into one active plan"
```

Expected: old plan files are archived under `C:\Users\soulzyn\Desktop\codex-backups` and removed from `docs/plans`.

**Step 3: Verify only one plan remains**

Run:

```powershell
Get-ChildItem docs\plans -File -Filter *.md | Select-Object Name
```

Expected: only `datamax-active-execution-plan.md`.

**Step 4: Check git diff**

Run:

```powershell
git status --short
git diff --check
```

Expected: one added active plan plus deleted historical plan files; no whitespace errors.

**Step 5: Commit**

```powershell
git add docs/plans
git commit -m "Consolidate DataMax active plan"
```

---

## Task 2: Current-Head Baseline Receipt

**Files:**

- Modify: `docs/validation/datamax-main-gap-closure-completion-audit.md`

**Step 1: Record local and 8-server state**

Run:

```powershell
git status --short --branch
git rev-parse --short HEAD
ssh 8服务器 'cd /srv/aiv3/repo && git status --short --branch && git rev-parse --short HEAD && systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-codex-host-agent.service aiv3-document-enrichment-worker.service aiv3-ingest-worker.service aiv3-retrieval-worker.service aiv3-static-page-worker.service'
```

Expected: local and 8-server match, except the known 8-server `?? mode`.

**Step 2: Run read-only queue and docs checks**

Run:

```powershell
ssh 8服务器 'curl -sS --max-time 8 http://127.0.0.1:3000/v1/workflow-tasks/queue-stats >/tmp/datamax-active-plan-qstats.json && python3 -m json.tool /tmp/datamax-active-plan-qstats.json >/dev/null'
npm run check:pure-third-party-guide-html
```

Expected: queue stats JSON is valid; docs are up to date.

**Step 3: Append a short receipt**

Add a dated section to `docs/validation/datamax-main-gap-closure-completion-audit.md` with:

- local/GitHub head;
- 8-server head;
- active service list;
- queue-stats reachable yes/no;
- known `mode` untouched;
- no secret or raw payload recorded.

**Step 4: Commit**

```powershell
git add docs/validation/datamax-main-gap-closure-completion-audit.md
git commit -m "Record DataMax active baseline"
```

---

## Task 3: Background Enrichment And Dedup Diagnostics

**Files:**

- Inspect/modify: `crates/platform-api/src/lib.rs`
- Inspect/modify: `crates/storage/src/lib.rs`
- Inspect/modify: `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- Inspect/modify: `apps/web/app/lib/external-integrations.js`
- Test: `apps/web/app/lib/external-integrations.test.mjs`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Goal:** Operators should be able to tell whether a document is canonical, duplicate, parsed, indexed, enriched, waiting, or blocked, without exposing raw document content.

**Step 1: Locate current diagnostics**

Run:

```powershell
rg -n "dedup_state|canonical_document_id|document_enrichment|enrichment_runs|fingerprint|parse_status|index_status" crates apps docs -g "*.rs" -g "*.js" -g "*.mjs" -g "*.md"
```

Expected: find existing storage fields, enrichment run helpers, and observability helpers.

**Step 2: Add or extend a redacted summary helper**

If missing, add a helper that returns only:

- document id;
- canonical document id;
- dedup state;
- parse/index status;
- enrichment run counts by kind/status;
- last error code/message summary;
- next eligible run time;
- no title/body/path/URL/raw metadata.

**Step 3: Write focused tests**

Add tests that prove:

- duplicate aliases show canonical read-through status;
- missing fingerprint appears as a blocked reason;
- summary does not include raw local paths, URLs, content, or credentials.

Run targeted tests, for example:

```powershell
cargo test -p platform-api document_enrichment --lib
cargo test -p platform-api canonical_duplicate --lib
node --test app/lib/external-integrations.test.mjs
```

Run the JS test from `apps/web`.

**Step 4: Update validation**

Record the sanitized summary shape in `docs/validation/datamax-main-gap-closure.md`.

**Step 5: Commit**

```powershell
git add crates apps docs/validation
git commit -m "Expose document enrichment diagnostics"
```

---

## Task 4: Duplicate And Canonical Read-Through Smoke

**Files:**

- Create or modify: `scripts/smoke/document-dedup-readthrough.mjs`
- Test: `crates/platform-api/src/lib.rs`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Goal:** Prove exact duplicate content can be authorized through different document/dataset scopes while retrieval/facts read through canonical content.

**Step 1: Add a local smoke script**

The script should run in local/test mode by default and avoid real customer documents. It should:

- create or reuse a disposable dataset/thread;
- register two documents with identical content bytes;
- assert two public document identities exist;
- assert one canonical content fingerprint exists;
- query evidence/facts through both identities;
- assert duplicate aggregation does not double-count facts.

**Step 2: Add dry-run mode for 8 server**

8-server mode must be read-only unless explicitly approved:

```powershell
node scripts/smoke/document-dedup-readthrough.mjs --base-url http://127.0.0.1:3000 --plan-only
```

Expected: reports what it would check without uploading.

**Step 3: Run local checks**

```powershell
node --check scripts/smoke/document-dedup-readthrough.mjs
cargo test -p platform-api canonical_duplicate --lib
```

**Step 4: Commit**

```powershell
git add scripts/smoke crates/platform-api/src/lib.rs docs/validation/datamax-main-gap-closure.md
git commit -m "Add document dedup readthrough smoke"
```

---

## Task 5: Passive Answer-Quality Offline Recovery Corpus

**Files:**

- Inspect/modify: `crates/platform-api/src/lib.rs`
- Inspect/modify: `crates/codex-host-agent/src/main.rs`
- Create or modify: `scripts/smoke/answer-quality-offline-corpus.mjs`
- Update: `docs/validation/datamax-main-gap-closure.md`

**Goal:** Improve the ability to find low-quality answers without blocking customer replies or enabling live auto-patch tasks.

**Step 1: Keep production hard gate off**

Run:

```powershell
rg -n "ASSISTANT_RUN_ANSWER_QUALITY_GATE_ENABLED|ASSISTANT_RUN_ANSWER_QUALITY_AUTOFIX_ENABLED|answer_quality_autofix" crates scripts docs
```

Expected: live autofix remains behind dedicated flags and allowlists.

**Step 2: Build an offline corpus smoke**

The smoke should classify stored or fixture answers into:

- likely evidence gap;
- likely retrieval scope gap;
- likely system wording gap;
- acceptable cautious answer;
- report/static-page action missed.

It must not enqueue Codex tasks or modify code.

**Step 3: Add fixtures from known customer patterns**

Include fixture prompts for:

- "资料不足/暂未找到" when the dataset has evidence;
- report trigger wording around `取高`, `经营状况`, `风险识别`, `销售缺口`, `助推`;
- ordinary non-report questions such as "取高是什么意思？";
- temporary document resume analysis.

**Step 4: Run tests**

```powershell
node --check scripts/smoke/answer-quality-offline-corpus.mjs
cargo test -p platform-api answer_quality --lib
cargo test -p platform-api assistant_run_answer_quality_gate --lib
cargo test -p codex-host-agent answer_quality --lib
```

**Step 5: Commit**

```powershell
git add scripts/smoke crates docs/validation
git commit -m "Add offline answer quality corpus"
```

---

## Task 6: Data-Source Row-Identity Staging Self-Test

**Files:**

- Modify: `scripts/run-data-ingestion-staging-live-smoke.sh`
- Modify: `docs/operations/data-source-row-identity-decision.md`
- Update: `docs/validation/data-ingestion-staging-sync-smoke.md`

**Goal:** Prepare row-level validation without changing production mappings.

**Step 1: Extend self-test fixtures**

Add synthetic fixtures that model:

- entity/latest snapshot identity;
- row-level detail identity;
- mixed identity with collapsed rows;
- missing stable row discriminator.

**Step 2: Emit recommended actions**

The smoke should recommend one of:

- keep entity/latest snapshot;
- add stable row discriminator in staging;
- split source into summary and detail datasets;
- do not claim row-level completeness.

**Step 3: Run local self-test**

```powershell
wsl.exe bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && bash -n scripts/run-data-ingestion-staging-live-smoke.sh"
wsl.exe bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true bash scripts/run-data-ingestion-staging-live-smoke.sh"
```

Expected: self-test emits no raw credentials and reports the correct recommendation for each fixture.

**Step 4: Commit**

```powershell
git add scripts/run-data-ingestion-staging-live-smoke.sh docs/operations/data-source-row-identity-decision.md docs/validation/data-ingestion-staging-sync-smoke.md
git commit -m "Strengthen data source identity self test"
```

---

## Task 7: Static-Page And Report Regression Corpus

**Files:**

- Modify: `scripts/smoke/external-report-export.mjs`
- Modify: `scripts/smoke/static-page-5way.mjs`
- Modify: `tools/validate-xinbai-report-template.mjs`
- Update: `docs/validation/static-page-render-smoke.md`

**Goal:** Keep the report/static-page path from regressing without relying on Image2 or Cloudflare for every test.

**Step 1: Add route fixtures**

Cover:

- should trigger report: `取高`, `经营状况`, `经营健康度`, `整体经营情况`, `风险识别`, `销售缺口`, `助推`, `哪些门店需要关注`, `坪效`, `客流统计`;
- should not trigger report: concept explanation, unrelated resume/system questions, plain nursing/manual questions;
- prompt focus maps to expected `?focus=` values.

**Step 2: Strengthen template validation**

Validator should check:

- one primary report link;
- report title `新世界百货经营管理月报表`;
- `table-data.csv`, `report.ppt`, `report.md`;
- focus parameter preserved;
- old fallback/prewarm/smoke templates not selected as normal default.

**Step 3: Run local and public checks**

```powershell
node --check scripts/smoke/external-report-export.mjs
node --check scripts/smoke/static-page-5way.mjs
npm run validate:xinbai-report-template
npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
```

**Step 4: Commit**

```powershell
git add scripts/smoke tools docs/validation/static-page-render-smoke.md
git commit -m "Strengthen report routing regression corpus"
```

---

## Task 8: Third-Party Database Read-Only Status Hardening

**Files:**

- Inspect/modify: `crates/platform-api/src/lib.rs`
- Inspect/modify: `apps/web/app/lib/database-source.js`
- Inspect/modify: `apps/web/app/lib/database-source.test.mjs`
- Update: `docs/validation/data-ingestion-analysis-smoke.md`

**Goal:** Improve the existing third-party-safe status view without opening new public registration/sync APIs.

**Step 1: Audit current public database docs and routes**

Run:

```powershell
rg -n "database-source|database_sources|data_ingestion|source readiness|third-party database|external.*database" crates apps docs -g "*.rs" -g "*.js" -g "*.mjs" -g "*.md"
```

**Step 2: Normalize redacted status**

Ensure the status shape distinguishes:

- source configured;
- connection test state;
- latest sync state;
- ready dataset count;
- default dataset empty vs older dataset ready;
- report/question readiness;
- row-identity warning present.

No raw table dump, SQL, URL, password, or source credential.

**Step 3: Tests**

```powershell
node --test app/lib/database-source.test.mjs
cargo test -p platform-api data_ingestion --lib
```

Run JS from `apps/web`.

**Step 4: Commit**

```powershell
git add crates apps docs/validation
git commit -m "Harden database source status summary"
```

---

## Task 9: Operator Observability Polish

**Files:**

- Modify: `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- Modify: `apps/web/app/lib/external-integrations.js`
- Test: `apps/web/app/lib/external-integrations.test.mjs`

**Goal:** Make active gaps visible without opening protected internals.

**Step 1: Add summary cards if missing**

The external integrations operations panel should summarize:

- answer-quality live autofix disabled/enabled;
- model-gateway authenticated smoke pending;
- historical enrichment real backfill disabled;
- row-identity warning for collapsed tables;
- static-page prewarm off by default;
- queue backlog count.

**Step 2: Tests**

```powershell
cd apps/web
node --test app/lib/external-integrations.test.mjs
npm run build
```

Expected: 27+ tests pass and build succeeds.

**Step 3: Commit**

```powershell
git add apps/web
git commit -m "Polish DataMax operations summary"
```

---

## Task 10: Final Validation And Optional Deploy

**Files:**

- Modify: `docs/validation/datamax-main-gap-closure.md`
- Modify: `docs/validation/datamax-main-gap-closure-completion-audit.md`

**Step 1: Run local release gate subset**

```powershell
cargo fmt --check
cargo check -p platform-api
npm --prefix apps/web run build
npm run check:pure-third-party-guide-html
npm run test:pure-third-party-guide-html
```

Add targeted tests from tasks actually changed.

**Step 2: Run 8-server read-only smoke**

```powershell
ssh 8服务器 'cd /srv/aiv3/repo && git status --short --branch && git rev-parse --short HEAD'
ssh 8服务器 'curl -sS --max-time 8 http://127.0.0.1:3000/v1/workflow-tasks/queue-stats >/tmp/datamax-active-plan-final-qstats.json'
```

If code changed and user asks to deploy:

```powershell
ssh 8服务器 'cd /srv/aiv3/repo && git pull --ff-only'
# Build/restart only changed services.
```

Do not deploy automatically unless requested.

**Step 3: Update completion audit**

Record:

- completed independent tasks;
- tests and receipts;
- external decisions still parked;
- no public API/auth/field changes unless explicitly approved.

**Step 4: Commit**

```powershell
git add docs/validation
git commit -m "Record DataMax active plan validation"
```

---

## Definition Of Done

This plan is current when:

- `docs/plans` contains only `datamax-active-execution-plan.md`;
- independent tasks above are implemented or explicitly marked not needed;
- local tests for changed code pass;
- 8-server read-only checks pass, and deploy happens only when requested;
- external-resource items stay parked with clear resume conditions;
- no raw credentials, raw customer rows, full documents, provider payloads, object paths, or tokens are recorded.
