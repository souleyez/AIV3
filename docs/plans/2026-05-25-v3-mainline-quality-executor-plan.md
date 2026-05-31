# V3 Mainline Quality And Executor Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep one active V3 mainline plan for parsing/answer quality, queryable facts, third-party static-page generation, data-ingestion analysis, and Codex executor boundaries.

**Architecture:** V3 remains the system of record for documents, datasets, permissions, task state, artifacts, and public integration contracts. Internal quality recovery, fact aggregation, Image2/static-page generation, and Codex executor work must feed back through V3-owned evidence, events, and artifact manifests. Customer-facing answer blocking stays conservative until smoke proves low false positives.

**Tech Stack:** Rust `platform-api`, `contracts`, `storage`, `assistant-runtime`, `codex-host-agent`; PostgreSQL fact tables and AssistantRun events; Next.js external integration docs; Cloudflare/Image2 queue; 8-server private smoke.

---

## Current Baseline - 2026-05-26

- Local, GitHub, and 8-server HEAD before the current static-page readiness batch: `2091c03`.
- 8-server services: `aiv3-platform-api.service` and `aiv3-web.service` are active.
- 8-server release builds must use `CC=clang CXX=clang++`; default `gcc 10.2.1` is rejected by `aws-lc-sys`.
- Third-party document authorization supports:
  - single `dataset_external_id`;
  - multiple `dataset_external_ids`;
  - explicit `available_document_external_ids`;
  - group + document union with duplicate documents ignored.
- Third-party static-page requests now support the simplified fields `artifact_type: "static_page"` and `template`, while old `render_mode` / `output_format` / `requested_skills` behavior remains compatible.
- Static-page/Image2 flow can queue an effect image, stream the preview/status card, continue without customer confirmation, and publish a new generated artifact when the fixed Codex task succeeds.
- Fixed Codex task directions exist for:
  - `static_page_image2_data_publish`;
  - `answer_quality_autofix`;
  - `data_ingestion_analysis`.
- Queryable fact tables and snapshots are populated on 8 server, but current private smoke must be rerun on the latest deployed HEAD.

## Active Guardrails

- Do not change third-party public URLs, auth, or request/response fields unless the operator explicitly approves.
- Do not re-enable a hard customer-facing answer quality gate yet; previous gate behavior blocked too many normal answers.
- Do not use VLM by default. VLM reparse is premium recovery only, budget-gated, and must become internal evidence before customer answers use it.
- Do not let Codex executor mutate datasets, document ownership, credentials, deployment config, or public integrations directly.
- Static-page no-confirm flow may create a new generated artifact only; it must not overwrite an existing customer artifact or stable URL without confirmation.
- Keep raw credentials, database URLs, provider payloads, full stdout/stderr, and customer file dumps out of prompts, events, docs, and artifact manifests.
- Keep HTML route selection explicit: chat answers that only need large tables/detail pages use V3 safe rapid HTML artifacts; only explicit static-page/product-page requests enter the Image2/Codex publish pipeline.

## Execution Notes - 2026-05-25

- Consolidated old active quality/executor plans into this single plan and pushed commit `d6381ac`.
- Local priority smoke passed on HEAD `d6381ac` for one-character PDF, DOC/DOCX "邓工是谁", resume company statistics, resume ranking table, attendance date/work-hour formatting, frequent attendance query, smart-home partial-parse answer quality, and smart-elevator point-list table output.
- Available 8-server fact snapshot probe passed against `https://v3.elepcloud.com`: resume company statistics used `dataset_fact_snapshot` with retrieval evidence as secondary supply and answered from 25 visible documents / 100 company-or-organization rows.
- Full private 8-server smoke now has a local ignored `ServerCaseConfigPath` generated from 8-server metadata. The first run exposed one real gap and two assertion issues:
  - direct AssistantRun dataset-only selected scope was overwritten by ordinary-chat scope for "邓工是谁";
  - real attendance rows use `2026-05-20`, `2026-02-07`, `2026-03-18`, `12.55小时`, and `4.05小时`, so private assertions must follow the real table;
  - resume ranking and smart-elevator server answers supplied `dataset_fact_snapshot` plus `dataset_entity_scan`, so private aggregate-source assertions should allow both.
- Third-party `dataset_external_ids` conversation scope on 8 server now resolves the e64 group to three authorized documents and the latest external-channel "邓工是谁" run supplied retrieval evidence from `资料1.docx`.
- Added internal supply selection notes in existing `supply_quality.notes` and smoke summaries, without adding third-party public request/response fields:
  - `supply_selection:dataset_fact_snapshot_selected_for_dataset_aggregate_question`;
  - `supply_selection:document_facts_scoped_aggregate_selected_for_scoped_document_aggregate`;
  - `supply_selection:dataset_entity_scan_kept_for_dimensions_not_covered_by_snapshot`;
  - `supply_selection:dataset_entity_scan_selected_when_snapshot_missing_or_runtime_scan_needed`;
  - `supply_selection:spreadsheet_row_analysis_selected_for_attendance_or_workhour_table_question`.
- Deployed `b901be3` and `014251d` to 8 server. The second deploy preserved 8-server public-doc line-ending drift in stash `pre-deploy public docs line endings 2026-05-25` before fast-forwarding.
- Full private 8-server smoke passed on deployed HEAD `014251d`:
  - one-character PDF low-text handling;
  - third-party DOC/DOCX "邓工是谁";
  - resume company-name statistics;
  - multi-dimension resume ranking table;
  - attendance absence / work-hour length / date formatting;
  - frequent attendance query;
  - smart-home customer dissatisfaction query;
  - smart-elevator point-list table query.
- Smoke script now reads HTTP error bodies through PowerShell `ErrorDetails.Message` / `HttpResponseMessage.Content` compatibility paths, so real server errors are not hidden by local response-object differences.
- Continued on 2026-05-26 by tightening static-page `static_page_image2_data_publish` auto-publish readiness:
  - V3 now treats Codex auto-publish as ready only when the platform task switch, platform allowlist, Codex Host agent capability allowlist, real execution mode, real Codex execution permission, trusted host kind, and task workspace root are all present.
  - If readiness is incomplete, the static-page path keeps the existing built-in HTML direct-render fallback instead of leaving users waiting on an effect-image-only task.
  - Static-page cards and SSE payloads expose additive diagnostics `codex_auto_publish_ready` and `codex_auto_publish_disabled_reason`; third parties can ignore these unless they are logging or debugging integration state.
  - Local regression passed for `external_channel_static_page`, full `external_channel`, and third-party guide HTML check.
- Follow-up on 2026-05-26: the preferred ready state is now `CODEX_HOST_AGENT_EXECUTION_MODE=cloudflare_orchestrator` with `CODEX_HOST_AGENT_HOST_KIND=cloudflare_codex` and a Codex Web orchestrator key. The old local-host `codex_exec` gate remains supported, but the fixed Cloudflare Codex executor no longer requires a local task workspace because V3 publishes returned HTML into its own generated-artifacts surface.
- Continued queryable fact aggregation on 2026-05-26:
  - Added scoped fact aggregate regressions proving selected-document scopes and third-party external temporary dataset scopes supply `document_facts_scoped_aggregate`.
  - The regressions prove out-of-scope documents in the same source dataset are excluded from company/organization aggregates.
  - Local regression passed for `external_channel`, `dataset_fact_snapshot`, and `scoped_fact`.
- Continued on 2026-05-27 by closing the centralized observation page lazy-access gap:
  - Codex executor tasks remain default-collapsed; opening the panel only loads the lightweight task list, and selecting one task loads runtime inspect.
  - The protected access form now preserves whether the operator was unlocking conversation tests or Codex executor observation, so the page returns to the intended lazy-loaded panel instead of always opening conversation tests.
  - Local web production build passed after the change.
- Continued on 2026-05-28 for the elderly-care manual demo path:
  - 8-server read-only probe found two `养老机构精细化运营实操手册（2026版V2）(1).doc` records; both are `parsed`, have 290 chunks, and have `fact_index.status=indexed` with 256 document facts.
  - The same probe showed the first fact-index pass was too directory-heavy: the 256 fact cap was mostly consumed by TOC dot-leader terms and generic service/organization ngrams before later nursing sections.
  - Tightened local fact extraction so post-ingest cleanup now collects candidates across the full document, filters TOC dot-leader noise, then ranks higher-value care-domain facts before applying the cap.
  - Added a nursing-handover retrieval ranking guard so `护理交接班时，必须交接的内容有哪些？` prefers the concrete `四、交接内容` chunk over generic `交接班制度` / `床旁交接班` references.
  - Focused local regressions passed for fact-index TOC noise, nursing handover, and the neighboring elderly-care retrieval ranking cases for 翻身、发药、跌倒.
- Continued on 2026-05-29 by turning the Xinbai dynamic static-page lesson into an internal V3 contract:
  - Static-page export packages now declare both `data-snapshot.json` and `data.json`; `data.json` is the client-refresh data entry for generated pages, while `data-snapshot.json` remains the renderer/source-of-truth handoff file.
  - Renderer manifests and queued export manifests now carry a `dynamic_page_contract` with time selector, primary partition selector, manual refresh, auto-refresh, 60-second polling, and snapshot/version change-detection expectations.
  - `static_page_image2_data_publish` fixed-task packages now instruct Cloudflare Codex to produce final HTML that can load local `data.json` and support time/primary-partition controls. This is internal task guidance only; public third-party fields stay unchanged.
  - V3's Codex Host generated-artifact publisher can persist returned `data_json` as `data.json` / `data-snapshot.json`, strip the large inline data from the fixed-task output, and publish only artifact paths/URLs plus the manifest.
  - Local validation passed for the export-artifact validator, static-page renderer unit, Codex Host publication unit, contracts fixed-task example, platform data-snapshot render regression, and the full static-page render smoke.
- Continued on 2026-05-30 by tightening the static-page data snapshot itself:
  - Draft `dataSnapshot` now carries `snapshotVersion` / `updatedAt`, a `data.json` refresh policy, module validation summary, sample/detail row counts, and metric/unit hints before the page enters Image2 or final HTML rendering.
  - Database aggregate bindings preserve metric unit hints into field candidates and sample rows, so database-backed reports can expose row count, detail count, snapshot date, and unit smoke signals.
  - Focused local regressions passed for `static_page_data_snapshot_binds_database_aggregate_rows` and the full `static_page_data_snapshot` test slice.
- Continued the same batch by expanding the read-only database live smoke:
  - `run-data-ingestion-staging-live-smoke.sh` now reports whether a synced database-derived dataset is ready for follow-up Q&A and static-page report generation, including the basis dataset, ready source tables, safe suggested questions, and the report smoke command.
  - This remains a V3-state-only smoke: it reads stored V3 source/sync/dataset/chunk/evidence status and does not connect to the customer/source database or execute customer-facing questions automatically.
  - The same script now supports `DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true`, which validates the report builder with synthetic V3 status fixtures when Postgres or the 8-server source is not available.
  - `run-data-ingestion-staging-sync-smoke.sh` now includes that live-source readiness self-test by default, so local staging sync smoke also guards the database Q&A/report readiness report shape.
- Continued P0 dataset document movement cleanup on 2026-05-30:
  - The main web app now derives document-membership active state from the selected document's own `dataset_ids` / `datasetIds` while editing document归属, rather than reusing ordinary chat selected dataset scope.
  - This keeps the left dataset rail and membership toggle direction aligned with the actual document memberships, so clicking a joined dataset sends remove and clicking an unjoined dataset sends add.
  - AssistantRun response handling no longer unions backend-selected scopes into the left rail when this turn already has an effective selected dataset scope, preventing an explicit 新百/report selection from visually expanding to every visible dataset.
  - Verified with `cargo test -p platform-api document_dataset_membership_endpoints_allow_main_site_public_document_moves --lib`, `cargo test -p platform-api assistant_run_native_empty_preselection_broadens_to_visible_supply --lib`, `npm --prefix apps/web run build`, and `git diff --check`.
- Continued P0 static-page handoff cleanup on 2026-05-30:
  - Third-party static-page requests now create a V3 direct HTML/generated-artifact link even when fixed Cloudflare Codex publish is ready, so `public_url` / `artifact_links[0]` can be returned quickly while the Image2 + Codex final page continues in the background.
  - The response and status-restored cards preserve `provisional_direct_html=true`, `codex_final_status=static_page_image2_auto_publish_pending`, `poll_after_seconds`, and the same `status_url`, so third parties can send the first page immediately and still poll for the later final publish.
  - The pure third-party and full third-party integration docs describe the provisional-link behavior, new card fields, and no-confirm Image2-to-Codex flow.
- Continued P0 third-party privacy cleanup on 2026-05-30:
  - External document parse still creates/reuses private third-party-owned datasets by default, and now also hardens legacy public duplicate datasets when the old dataset only contains same-source third-party documents.
  - This keeps old public duplicate document shells out of the standard main-station dataset and document lists after a third-party account re-uploads the same external document id.
  - Verified with `cargo test -p platform-api external_document_parse --lib`.
- Continued P0 selected-scope recovery on 2026-05-30:
  - Ordinary main-station chat now drops stale or inaccessible requested dataset ids from `selected_scope` instead of carrying a dirty selected state into evidence planning or later ReAct steps.
  - Existing data-question behavior remains: empty selected/preselected scopes can broaden to visible datasets with `dataset_scope_policy=all_visible_datasets_with_preselection_priority`.
  - Verified with `cargo test -p platform-api assistant_run_ordinary_chat_drops_unavailable_selected_dataset --lib` and `cargo test -p platform-api assistant_run_native_empty_preselection_broadens_to_visible_supply --lib`.
- Continued weak-retrieval procedure expansion on 2026-05-30:
  - Elder-care death/after-death procedure prompts now expand across `离世/去世/死亡/身故/善后/遗体/遗物/生命体征/医护确认/通知家属/殡仪接运/遗物交接/记录归档`.
  - Ranking now gives domain hints to death/after-death procedure sections, so `长者在院离世，现场处置、家属对接流程` prefers actual善后处置 evidence over generic家属探望/物品交接 text.
  - Verified with `cargo test -p platform-api rank_document_chunks_for_prompt_expands_elder_death_procedure_terms --lib`, `cargo test -p platform-api lexical_query_term_weights_extend_elder_death_terms --lib`, and the existing elder-fall ranking regression.
- Continued TOC/section-hop retrieval recovery on 2026-05-30:
  - Procedure fallback ranking now extracts clean section title hints from dotted TOC lines, uses those titles as query-hop terms, and downranks TOC/index chunks so actual section content wins when available.
  - This targets cases where the directory says `突发事件应急预防与处置` but the answer previously only cited the directory or a nearby sign/definition page.
  - Verified with `cargo test -p platform-api rank_document_chunks_for_prompt_uses_toc_title_to_prefer_actual_section --lib` plus the elder-fall and elder-death procedure regressions.
- Continued answer recovery guidance on 2026-05-30:
  - Weak or empty document supply now adds a model-facing `recovery_followup` with a concrete same-conversation follow-up and next-action hint, instead of letting the model end on a bare "not found" statement.
  - The model supply brief and ReAct natural fallback input both surface that recovery follow-up, requiring one useful answer first when any evidence exists, then a precise follow-up only if exact verification still needs more input.
  - Answer-quality autofix case packages now preserve the recovery follow-up context for later diagnosis.
- Continued third-party output-format enforcement on 2026-05-30:
  - External-channel answer policy now emits explicit model-facing guidance for `rich_text`, `image_text`, `markdown_table`, and `json`, instead of relying only on the raw policy JSON blob.
  - ReAct compact natural fallback now distinguishes customer JSON output from internal JSON leakage, so `output_format=json` no longer conflicts with the internal "do not output observation/execution JSON" guard.
  - Deterministic answer-quality fallback tables now honor `output_format=json` for spreadsheet row analysis and point-list/elevator rows, rather than returning Markdown tables on JSON-only third-party turns.
- Continued static-page visual asset hardening on 2026-05-30:
  - Effect-image completion now records a safe preview-asset provenance summary alongside the stable V3 preview URL, including source kind, redacted source reference, persisted URL, byte size, mime type, dimensions, storage status, and orchestrator task id.
  - Embedded `data:image` payloads and signed remote query strings are redacted from manifests, so final HTML generation can rely on V3-owned preview assets without leaking upstream temporary URLs.
  - The third-party Image2 fixed task now passes `render_asset_url` plus the safe provenance summary into Codex Host, and `prompt_text` no longer serializes the raw image payload where signed source URLs can appear.
  - Verified with `cargo fmt --package static-page-worker --check`, `cargo test -p static-page-worker`, `cargo test -p platform-api external_channel_static_page_fixed_task_packages_scope_and_policy --lib`, and `cargo test -p codex-host-agent cloudflare_orchestrator_fixed_task_prompt_is_hard_bounded -- --nocapture`.
- Continued deployment readiness smoke cleanup on 2026-05-31:
  - `run-cloudflare-codex-fixed-task-smoke.ps1` remote mode no longer treats `/healthz` frontend redirects or HTML as backend health.
  - The guarded 8-server readiness check now verifies the public third-party guide, the external events auth guard, and workflow queue JSON diagnostics without server mutation.
  - Verified with `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -PlanOnly -Case static-page-no-confirm,data-ingestion-analysis -Json`.
- Continued workflow queue diagnostics on 2026-05-31:
  - Queue stats now expose finished-task duration P50/P95 by logical queue and task key, giving the observability page/API enough data to separate queue depth from actual runtime.
  - The external integrations Codex executor panel now displays those P50/P95 runtime metrics in the lazy-loaded queue snapshot.
  - Queue stats now also expose success-only duration P50/P95, and the observation UI prefers those values when available so old failed/stale workflow tasks do not distort the normal successful-runtime signal.
- Continued static-page data repair on 2026-05-31:
  - Static-page生图和最终渲染入口会先刷新 V3 数据合同，第三方传来的旧 image prompt payload 也会用草稿里的最新供料上下文重建快照。
  - 数据库聚合行现在会随字段候选携带为 sampleData；`chartOptions.dataKey` / `bindingQuality.fieldPath` 被识别为可修复绑定，避免把可先生成再调整的图表误判成阻断。
- Continued fixed-task smoke hardening on 2026-05-31:
  - `run-cloudflare-codex-fixed-task-smoke.ps1` now supports reviewed real `static-page-no-confirm` third-party mutation smoke against 8服务器, using `-BearerToken`, `V3_EXTERNAL_CHANNEL_BEARER_TOKEN`, or private `ServerCaseConfigPath.bearer_token_env`.
  - The real smoke builds a third-party static-page event, polls the returned status URL when present, and reports redacted status/artifact fields without printing credentials.
  - Missing-bearer guard was verified: `-AllowServerMutation` fails before sending the mutation request with `mutation_attempted=false`.
  - The same guarded pattern now covers `data-ingestion-analysis`; it also requires a private source selector before mutation and reports `source_configured=false` when the case config is incomplete.
- Continued real 8-server mutation smoke on 2026-05-31:
  - Static-page no-confirm passed end-to-end through the real third-party `/events` endpoint and returned a generated-artifact URL immediately.
  - Data-ingestion analysis initially exposed a production config gap: `data_ingestion_analysis` was missing from both platform and Codex Host agent fixed-task allowlists. The 8-server env was backed up, the capability was added to both allowlists, and `aiv3-platform-api.service` / `aiv3-codex-host-agent.service` were restarted.
  - After the config fix, data-ingestion analysis accepted the request and moved through `queued` to `running/retrying` with no source-required or allowlist rejection; terminal result still needs follow-up polling.

## Immediate Execution Queue

1. **Fix quality regressions in source order**
   - First fix missing or wrong deterministic supply.
   - Then fix table/date/unit formatting.
   - Then fix selected-document detail or document scope restoration.
   - Then fix fact snapshot/scoped aggregate supply.
   - Only after those, adjust ReAct or quality-gate retry behavior.

2. **Queryable fact aggregation**
   - Verify full dataset/group statistics prefer `dataset_fact_snapshot`.
   - Verify selected-document, ACL-filtered, and temporary conversation scopes can use `document_facts_scoped_aggregate` for supported company/skill/project/keyword/year/section prompts. Selected-document and external temporary scope regressions are now covered locally; keep extending with real 8-server smoke cases as customer data appears.
   - Keep attendance on `spreadsheet_row_analysis` and resume ranking on resume-profile deterministic rows until their facts are normalized.
   - Add a visible debug reason for why `dataset_fact_snapshot`, `document_facts_scoped_aggregate`, `dataset_entity_scan`, or `spreadsheet_row_analysis` was selected.

3. **Answer quality recovery**
   - Keep customer-facing output permissive and avoid false-positive blocking.
   - Collect suspicious low-quality replies into `answer_quality_autofix`.
   - Classify issues as missing source data, poor parsing, weak retrieval/supply, or answer-generation logic.
   - Restrict any automatic code modification to answer-quality/retrieval/supply optimization scope.

4. **Codex executor boundary**
   - Consolidate a shared executor input envelope across fixed templates.
   - Consolidate a shared artifact manifest shape for static pages, Image2 previews, reports, and data-ingestion analysis.
   - Make failed/retrying executor task statuses consistently model-visible.
   - Add lazy observation surfaces that do not poll every conversation or task.
   - Keep fixed-task auto-execution gated by explicit runtime readiness instead of platform allowlist alone.

5. **Static-page productization**
   - Treat Image2 as a visual contract/reference, not final HTML source of truth.
   - Do not send ordinary chat detail pages, resume matrices, evidence tables, or answer appendices through Image2. Those stay on the rapid HTML artifact path unless the user explicitly asks for a designed/published static page.
   - Generate final HTML from structured real-data snapshots and validated source summaries.
   - Publish designed static pages with `data.json` as the dynamic refresh entry and `data-snapshot.json` as the renderer handoff snapshot.
   - Keep default page requirements:
     - time selector;
     - primary partition selector, such as store/region/project;
     - refresh from updated documents/data;
     - detail tables for business-critical rows.
   - Add version, refresh, unit, snapshot-date, and detail-count smoke before making stable overwrite/publish automatic.

6. **Data ingestion analysis**
   - Customer data接入/入库/建表/字段映射/schema/ETL/清洗 requests may queue `data_ingestion_analysis` from existing chat/event fields when V3-selected source scope exists.
   - Missing source scope returns `data_ingestion_analysis_source_required`.
   - Real mutation smoke remains operator-approved and guarded.

## Smoke Commands

Local priority smoke:

```powershell
.\scripts\run-v3-quality-gate-smoke.ps1 -Local -Case @(
  'one_character_pdf',
  'deng_engineer',
  'resume_company_stats',
  'resume_ranking_table',
  'attendance_final',
  'attendance_frequent',
  'smart_home',
  'smart_elevator'
)
```

Private 8-server smoke:

```powershell
.\scripts\run-v3-quality-gate-smoke.ps1 `
  -ServerBaseUrl https://v3.elepcloud.com `
  -ServerCaseConfigPath <private-case-config.json> `
  -Case @(
    'one_character_pdf',
    'deng_engineer',
    'resume_company_stats',
    'resume_ranking_table',
    'attendance_final',
    'attendance_frequent',
    'smart_home',
    'smart_elevator'
  )
```

Focused regression tests:

```powershell
cargo test -p platform-api external_channel --lib
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api codex_host_fixed_task --lib
cargo test -p contracts external_bot_message --lib
npm run build:pure-third-party-guide-html
npm run test:pure-third-party-guide-html
npm run check:pure-third-party-guide-html
```

8-server deploy build reminder:

```bash
cd /srv/aiv3/repo
git pull --ff-only origin main
CC=clang CXX=clang++ cargo build --release -p platform-api
cd apps/web
pnpm build
systemctl restart aiv3-platform-api.service
systemctl restart aiv3-web.service
```

## Consolidated Plans

Top-level product and engineering roadmap now lives in `docs/plans/2026-05-30-v3-complete-development-plan.md`. Use that file first for priority, product boundary, release rules, and cross-workstream ordering; use this file for the active mainline execution queue and smoke history.

This plan supersedes the prior active parsing/answer-quality, quality-gate/ReAct/VLM, queryable-fact-index, fixed Codex escalation, and executor-boundary plans.

Those old active plan files were removed from the active plan folder after backup. Git history remains the detailed archive.

## Active Subplans

- `docs/plans/2026-05-26-codex-executor-gap-closure-plan.md`: closes the post-executor-integration gaps for fixed-task model-facing status, static-page publish callback, data-ingestion handoff, answer-quality autofix review gates, lazy observability, retry/timeout/cancellation policy, and staged smoke before real production enablement.
