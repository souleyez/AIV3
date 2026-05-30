# V3 Complete Development Plan Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep one complete product-and-engineering plan for V3 so daily feature work, quality fixes, third-party integration, static-page/report generation, database-source ingestion, Codex executor work, and 8-server release smoke all follow the same priority order.

**Architecture:** V3 remains the system of record for tenants, datasets, document visibility, AssistantRun state, workflow state, artifacts, generated pages, third-party contracts, and audit. Documents, databases, user history, and generated artifacts all enter the same dataset-centered evidence chain before they can affect answers, reports, or static pages. Codex/Cloudflare executors are bounded workers that return artifacts and diagnostics through V3; they do not own permissions, credentials, dataset mutation, third-party API contracts, or release configuration.

**Tech Stack:** Rust `platform-api`, `contracts`, `storage`, `assistant-runtime`, `llm-gateway`, `ingest-worker`, `retrieval-worker`, `static-page-worker`, `codex-host-agent`; PostgreSQL; Next.js V3 web app; third-party integration docs; Cloudflare Codex/Image2; 8-server deployment and smoke scripts.

---

## How To Use This Plan

This is the top-level development plan. Use it as the first document when deciding what to do next.

- Daily execution detail stays in `docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md`.
- Codex executor closure detail stays in `docs/plans/2026-05-26-codex-executor-gap-closure-plan.md`.
- Database-source detail stays in `docs/plans/2026-05-21-database-source-integration-plan.md`.
- Parser/document-understanding detail stays in `docs/plans/2026-05-20-v3-context-document-understanding-p0.md`.
- Model-pool concurrency detail stays in `docs/plans/2026-05-21-model-gateway-concurrency-plan.md`.
- Background enrichment/dedup detail stays in `docs/plans/2026-05-28-v3-background-document-enrichment-dedup-plan.md`.
- Cloudflare Codex production bridge detail stays in `docs/plans/2026-05-26-cloudflare-codex-production-bridge-dev-doc.md`.

When these documents conflict, follow this plan for priority and product boundary, then update the lower-level plan that contains the implementation detail.

## Current Baseline

- Local branch is ahead of `origin/main` by one commit: `bdd8497 Tighten static page data readiness smoke`.
- Main active plan exists, but previous work was spread across multiple specialist plans.
- 8 server remains the active production/demo target for V3 智能助手.
- 120 server is no longer a sync target.
- Third-party public contracts must remain backward-compatible unless the operator explicitly approves a breaking change.
- Recent work has already landed around:
  - third-party temporary document scopes and multi-dataset document authorization;
  - template skill and requested skill handling;
  - simplified third-party docs;
  - static-page Image2/Codex auto-publish readiness;
  - `data.json` / `data-snapshot.json` dynamic page contract;
  - database-source status and readiness smoke;
  - dataset fact snapshots and scoped document aggregates;
  - external observability lazy loading;
  - private document behavior and dataset document movement.

## Non-Negotiable Product Rules

- V3 answers must be model-authored. Do not return fixed orchestration copy as the final user answer.
- If an answer cannot be completed, the assistant must either produce the best evidence-backed answer or ask a concrete follow-up that can actually change the next retrieval/tool action.
- Dataset selection is a supply preference and speed hint, not a hard global exclusion. If the selected dataset is missing or irrelevant, V3 may broaden within the current user's visible permissions.
- Third-party document scope is authoritative for that conversation. If a third party passes document IDs or dataset IDs once for the same conversation, V3 should retain the effective scope unless the third party changes it.
- Third-party enterprise documents are private by default and must not become visible in the public/main dataset surface unless an authorized operator explicitly moves or shares them.
- Database sources are external inputs, not live model tools. Rows must be synchronized into a V3 dataset or summarized into V3-owned evidence before chat/report/static-page use.
- Static-page Image2 is a visual contract. Final pages must be generated from V3 structured data snapshots and validated artifacts.
- Static-page generation should not block on customer confirmation after the initial prompt/template stage unless the user explicitly asks for review.
- Codex executor can generate or repair artifacts, but it must not mutate datasets, permissions, credentials, deployment config, or public integration contracts.
- VLM fallback is premium and budget-gated. PaddleOCR/structured parsing remains the preferred low-quality parse recovery path.
- No raw credentials, database URLs, API keys, source cursors, full customer dumps, or raw provider logs in prompts, docs, events, or generated artifacts.

## Product Capability Map

| Area | Current Role | Target |
| --- | --- | --- |
| Document ingestion | Upload, third-party parse, dataset membership, indexed chunks/facts | Background quality-aware parsing, canonical dedup, async enrichment, section/entity/noun-term understanding |
| Dataset and permissions | Visible datasets, document memberships, third-party temporary scopes | Stable multi-dataset membership, private enterprise documents, selected-scope persistence, source-scoped datasets |
| AssistantRun Q&A | Dataset-aware answer path with retrieval/fact supply | Must-answer-or-actionable-follow-up behavior, expanded retrieval, deterministic facts for stats/tables/ranking |
| Third-party integration | Events, stream, parse, templates, output format, static page task cards | Minimal docs, stable scope memory, dataset/document movement, status polling, generated artifact links |
| Static pages/reports | Drafts, Image2 preview, Codex publish, dynamic data contract | No-confirm publish, data-rich reports, user-adjustable final page links, role-specific report variants |
| Database sources | Managed MySQL source inspection/sync/status/readiness | Customer source onboarding, schema/profile mapping, synced datasets, report-ready semantic profiles |
| Codex executor | Fixed templates and Cloudflare bridge | Reliable artifact generation, task status supply, operator observation, bounded retries and timeouts |
| Model pool | Planned/parallel work | Concurrency for at least 10 users, profile routing, timeouts, fallback, health visibility |
| Observability | Centralized integration page with lazy panels | External conversations, executor tasks, database source status, no idle resource drain |

## Priority Roadmap

### P0: Demo And Production Stability

1. Keep 8 server deployable and smokeable after every meaningful batch.
2. Ensure static-page/report requests from third-party return a final `public_url` or a truthful processing status that later resolves to a URL.
3. Remove user-visible dead ends such as "needs_sample_rows" when V3 can fetch more rows, repair bindings, or generate a partial page with follow-up adjustment guidance.
4. Stabilize selected dataset behavior in the main assistant: selected means selected; no separate "preselected" state in the user mental model.
5. Keep third-party document and dataset scopes persistent per conversation.
6. Fix dataset document membership operations from the data page: add document to dataset, remove document from dataset, and show all current dataset memberships.
7. Preserve third-party private document isolation and clean accidental public duplicates when detected.
8. Keep Cloudflare Codex static-page visual and final HTML path ready for demos.

### P1: Quality Mainline

1. Improve document understanding beyond hard chunks:
   - section/paragraph boundaries;
   - title hierarchy;
   - table structure;
   - nouns, entities, roles, dates, skills, projects, companies, people;
   - domain synonyms such as 摔倒/跌倒/意外伤害/突发事件/处置/120/家属通知.
2. Improve answer supply:
   - first deterministic facts;
   - then scoped aggregates;
   - then retrieval evidence;
   - then expanded retrieval when quality is weak;
   - then general model knowledge clearly labeled.
3. Make resume-style queries robust across dimensions:
   - company;
   - time;
   - skill;
   - project;
   - age;
   - gender;
   - education;
   - ranking and sortable tables.
4. Apply the same pattern beyond resumes to enterprise documents and database-derived rows.
5. Make quality failures feed `answer_quality_autofix` or operator diagnostics rather than customer-facing dead ends.

### P2: Data Source And Enterprise Understanding

1. Bring database sources forward as first-class V3 data sources.
2. Support managed mode first:
   - operator configures source;
   - raw credential stays server-side;
   - schema/profile/sync creates V3 dataset documents;
   - questions/reports/static pages consume synced evidence.
3. Later add third-party self-service database source APIs only after auth, credential storage, rate limits, and tenant isolation are explicit.
4. Generate customer-ready reports from mixed document + database datasets.
5. Support role-specific static pages, such as 总部 and 分店店总 versions, using the same data snapshot with role-filtered views.

### P3: Scale And Operator Experience

1. Finish model-pool routing for at least 10 concurrent users:
   - profile concurrency;
   - queue timeout;
   - sequential fallback;
   - health and usage visibility.
2. Finish Codex executor operator controls:
   - task list lazy load;
   - selected task inspect;
   - retry/cancel/timeout rules;
   - workspace retention policy.
3. Improve release hygiene:
   - smoke matrix by feature;
   - named real smoke records;
   - rollback notes;
   - release checklist before GitHub push and 8-server deploy.

## Workstream Plans

### Workstream A: 8 Server Release And Smoke

**Files:**
- Modify as needed: `scripts/run-v3-quality-gate-smoke.ps1`
- Modify as needed: `scripts/run-data-ingestion-staging-sync-smoke.sh`
- Modify as needed: `scripts/run-data-ingestion-staging-live-smoke.sh`
- Modify as needed: `docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md`

**Tasks:**
1. Run local focused tests for the touched area.
2. Run guide/static-page/database smoke when integration docs or artifacts change.
3. Push to GitHub only when requested.
4. Deploy to 8 server only when requested.
5. After deploy, run the private 8-server smoke cases and record result in the active plan.

**Acceptance:**
- 8 server services restart successfully.
- Recent customer cases do not regress.
- Failed smoke includes actionable logs, not a silent spinner.

### Workstream B: Document Parsing And Understanding

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Modify: `crates/retrieval-worker/src/main.rs`
- Modify: `crates/platform-api/src/fact_index.rs`
- Modify as needed: `crates/ingest-worker/src/main.rs`
- Test: focused parser/retrieval/fact tests in the same crates

**Tasks:**
1. Add failing tests for one real customer query before changing logic.
2. Prefer deterministic structure or fact improvements before prompt tuning.
3. Add synonym and intent expansion only when the evidence path is measurable.
4. Add section/table/entity extraction improvements to fact supply.
5. Keep VLM fallback behind parse-quality/budget gates.

**Acceptance:**
- Elderly-care procedure questions find section-level evidence, not only sign/definition pages.
- Resume and enterprise-document aggregates use scoped facts or document facts before broad retrieval.
- Low-quality parsing is visible to the model as status, not hidden as success.

### Workstream C: Answer Quality And Retrieval Recovery

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Test: `external_channel`, `dataset_fact_snapshot`, `scoped_fact`, and retrieval tests

**Tasks:**
1. Classify the question type: definition, detail, process, statistics, ranking, report, static page, database analysis.
2. Load deterministic supplies first when available.
3. If first retrieval is weak, expand with synonyms, shorter terms, title/TOC hops, and process keywords.
4. If still weak, answer with current evidence plus a concrete follow-up that can trigger a real next action.
5. Record suspicious cases for `answer_quality_autofix`.

**Acceptance:**
- No customer-facing "本轮没有生成回复" for recoverable model/provider failures.
- No final answer that only says "未直接检索到" when adjacent evidence can be found by expanded retrieval.
- Follow-up requests can actually continue from the previous state.

### Workstream D: Dataset, Document Membership, And Privacy

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `apps/web/src/**` dataset page files
- Test: dataset membership and external-channel visibility tests

**Tasks:**
1. Ensure selected document membership operations are atomic and idempotent.
2. Show all datasets that selected documents belong to.
3. Toggle membership by clicking a dataset: add if absent, remove if present.
4. Keep third-party enterprise documents private by default.
5. Avoid counting duplicate aliases twice in facts/statistics.

**Acceptance:**
- A document can belong to multiple datasets.
- Moving a document for third-party grouping does not break its stable external document ID.
- Main station cannot see third-party-private documents without permission.

### Workstream E: Third-Party Integration

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Modify: `docs/integrations/third-party-integration-api.zh-CN.md`
- Modify generated docs as required by existing scripts

**Tasks:**
1. Preserve existing URLs, auth, request fields, and response fields unless explicitly approved.
2. Keep the minimal docs focused on:
   - document parse;
   - chat sync with default prompt, user ID, conversation ID, output format;
   - generated artifacts/reports with template list, template parse, template generation;
   - all fields annotated.
3. Support output format choices:
   - rich text;
   - graphic layout;
   - Markdown table;
   - JSON.
4. Support document movement by stable `document_external_id` plus target dataset/group external id.
5. Keep per-conversation document scope sticky.

**Acceptance:**
- Third-party AI with small context can use the minimal page without reading background prose.
- Status polling can see queued/running/retrying/completed/failed artifact states.
- Static-page success returns `artifact_links` or `public_url`.

### Workstream F: Static Pages, Reports, And Templates

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/static-page-worker/src/main.rs`
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: `external_channel_static_page`, `static_page_data_snapshot`, `codex_host_fixed_task`

**Tasks:**
1. Treat template documents as skill inputs for structure, layout, and field organization.
2. Generate or repair `dataSnapshot` before Image2/final HTML when modules need rows or field matching.
3. Do not stop the user at the image stage by default.
4. Publish final HTML as a V3 generated artifact.
5. Include `data.json`, `data-snapshot.json`, validation summary, unit hints, row counts, and refresh metadata.
6. For role-specific pages, create separate views or links from the same validated data snapshot.

**Acceptance:**
- Third-party request can go from instruction to final static page link.
- Main station mirrors the same no-confirm path after the initial prompt/template.
- A generated page can be adjusted after creation through chat or module edits.

### Workstream G: Database Source Integration

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/external-source-worker/src/**`
- Modify: `apps/web/src/**` data source and observability files
- Test: database source/status/sync tests and staging smoke scripts

**Tasks:**
1. Keep managed mode as P0.
2. Store only redacted config and environment/secret references.
3. Support schema inspection, semantic profile, mapping, sync, status, and readiness.
4. Sync rows into explicit V3 datasets.
5. Feed synced datasets into Q&A, reports, templates, and static pages.
6. Delay public third-party database registration APIs until credentials and tenant isolation are fully specified.

**Acceptance:**
- A source can be understood without hot-polling or exposing credentials.
- Operators can see whether a database dataset is question/report ready.
- Static-page planning can bind database fields and sample rows.

### Workstream H: Codex Executor And Cloudflare Bridge

**Files:**
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `crates/static-page-worker/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/operations/**`
- Test: `codex_host_fixed_task`, static-page worker smoke

**Tasks:**
1. Keep fixed templates:
   - `static_page_image2_data_publish`;
   - `data_ingestion_analysis`;
   - `answer_quality_autofix`.
2. Keep readiness gates explicit.
3. Surface safe task status to model and operator.
4. Treat slow Cloudflare tasks as processing/retrying, not failed, unless remote state or V3 validation says failed.
5. Keep runtime inspection lazy and selected-task-only.

**Acceptance:**
- 8 server can call the fixed Cloudflare Codex runtime for static-page demos.
- Task status never exposes raw prompt, credentials, stdout/stderr, or customer dumps.
- Final artifacts return through V3 publication.

### Workstream I: Model Pool And Concurrency

**Files:**
- Modify: `crates/llm-gateway/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `apps/web/src/**` model pool page files
- Test: gateway scheduler and provider profile tests

**Tasks:**
1. Add UI-managed model profiles behind a safe fallback.
2. Enforce per-profile concurrency and queue timeout.
3. Add sequential fallback for retryable failures.
4. Expose health, active calls, queue depth, success/failure, and latency.
5. Tune for 10 concurrent demo users before broader rollout.

**Acceptance:**
- Different users can run through different available profiles concurrently.
- One request uses one selected profile at a time.
- Disabling DB profiles falls back to env configuration.

## Fixed Smoke Matrix

Run or maintain smoke coverage for these real cases:

| Case | Purpose |
| --- | --- |
| DOC/DOCX "邓工是谁" | Third-party selected document retrieval and answer |
| 简历公司名统计 | Dataset fact snapshot and scoped aggregate |
| 简历多维排序表 | Profile extraction and table generation |
| 老人摔倒后怎么办 | Process intent, synonym expansion, section hop |
| 长者在院离世流程 | Procedure retrieval and no-spinner failure recovery |
| 停车合同 | Contract/document answer reliability |
| 新百资料报表 | Template/static-page/data snapshot quality |
| 新百权限表 | Role-specific page/report variants |
| hy_sql/bi_traffic_area | Database-source understanding and static report |
| static_page_image2_data_publish | Cloudflare Codex image-to-HTML closure |

## Release Checklist

Before local commit:

```powershell
cargo fmt --package platform-api --check
npm run check:pure-third-party-guide-html
git diff --check
```

Add focused tests for the touched area, for example:

```powershell
cargo test -p platform-api external_channel --lib
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api static_page_data_snapshot --lib
cargo test -p platform-api dataset_fact_snapshot --lib
```

Before GitHub push:

- Confirm `git status --short` only contains intended files.
- Confirm generated docs are updated when integration docs changed.
- Confirm no raw credentials or customer dumps were added.

Before 8-server deploy:

- Confirm the target commit hash.
- Confirm no 120-server sync is planned.
- Confirm 8-server build environment uses `CC=clang CXX=clang++`.
- Restart only the services affected by the deployed change.

After 8-server deploy:

- Run the private smoke cases relevant to the change.
- Record the deployed commit and smoke result in `docs/plans/2026-05-25-v3-mainline-quality-executor-plan.md`.
- If smoke fails, keep a rollback note and do not hide the failure behind customer-facing fallback copy.

## Immediate Next Execution Order

1. Finish current unpushed local batch decision: push/deploy `bdd8497` only when explicitly requested.
2. Continue P0 static-page/public-url closure and no-confirm main-station flow.
3. Finish dataset document membership toggle reliability.
4. Tighten third-party private document visibility and duplicate handling.
5. Add answer recovery for selected-scope missing dataset and weak retrieval cases.
6. Continue database-source dataset/report readiness and static-page binding.
7. Move parsing/answer-quality deep work to the dedicated quality thread while the feature thread keeps product surfaces moving.

## Open Risks

- Static-page generation can still produce weak plans when the dataset has sparse rows or template requirements are ambiguous.
- Slow Cloudflare tasks need robust processing/retry semantics so demos do not look stuck.
- Parser/fact extraction improvements can change answer evidence ordering; keep tests anchored to evidence type and answer usefulness, not brittle wording.
- Model-pool changes should stay disabled/observe-only until production health and rollback are clear.
- Third-party database self-service APIs are not yet safe until credential handling and tenant isolation are fully specified.

