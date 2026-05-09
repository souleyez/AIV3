# AI Data Platform V3 Master Development Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Provide the single active development plan for AI Data Platform V3, with static-page generation as the product mainline and Codex Host/model proxy as optional execution extensions behind V3 control.

**Architecture:** V3 remains the control plane and source of truth: PostgreSQL owns identity, dataset visibility, AssistantRun state, memory scope, workflow state, and artifacts. The assistant supplies model context, evidence, tools, and state but does not locally compose final answers. Static-page generation, reports, parsing, retrieval, and media understanding stay as V3 product capabilities; Codex Host is an external audited worker, not a replacement for V3's API, data plane, model gateway, or worker plane.

**Tech Stack:** Next.js 16 / React 19 in `apps/web`; `react-grid-layout` for desktop module layout; `@dnd-kit` for mobile vertical ordering; Apache ECharts for advanced chart/runtime parity; deterministic HTML/SVG rendering for export-safe fallback; sandboxed HTML artifact renderer for task reports/planning handoffs/lightweight editors; Rust crates `platform-api`, `assistant-runtime`, `llm-gateway`, `static-page-runtime`, `static-page-worker`, `static-page-renderer`, `ingest-worker`, `retrieval-worker`, `memory-worker`, `document-vlm-runtime`, `codex-host-agent`; PostgreSQL 17.9 target; Cloudflare/Codex image queue; MiniMax VLM/media capability probes; OpenClaw only as optional legacy sidecar.

---

## Status

This is the current active plan as of 2026-05-07.

Older plan files are now detailed references, not independent roadmaps:

- `docs/plans/2026-04-27-static-page-generation-studio-plan.md`: detailed static-page, assistant shell, ingest, media, and renderer history.
- `docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md`: older consolidated handoff and status log.
- `docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md`: detailed separated-memory and Codex Host memory plan.
- `docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md`: detailed model routing and Codex Host bridge plan.
- `docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md`: detailed module-boundary plan for Codex Host.
- `docs/plans/2026-05-07-v3-email-account-auth-plan.md`: detailed account/email auth plan; account work is paused after current hardening unless explicitly resumed.

When old documents conflict with this one, follow this master plan.

## Product North Star

V3 is not a generic file manager and not a standalone page builder. It is a data-aware intelligent assistant that can:

- Chat normally when no dataset is selected.
- Understand visible datasets, documents, conversation memory, and artifacts as optional context supply.
- Let the model choose retrieval/tool actions through V3 validation.
- Generate static-page/report artifacts inside the main assistant workspace.
- Let users adjust modules by natural language and lightweight direct manipulation.
- Open safe HTML artifacts for planning handoff, execution reports, code review summaries, and lightweight JSON-patch editors.
- Preserve finished outputs and drafts in the right shelf.
- Keep permissions, memory, and artifacts scoped to user/account/local-key visibility.

## Non-Negotiable Rules

- No fake data. If evidence is missing, the UI and generated artifact must say data is missing or partial.
- No local answer composition. V3 supplies context and actions; the model writes the answer.
- Dataset selection is supply preference, not a separate chat mode.
- Conversation history is a hidden dataset and enters context only through scope policy.
- Foreground upload work only saves, preclassifies, registers, and enqueues. Heavy parsing is background work.
- Static-page planning lives in the main workspace, not a separate popup.
- The right panel remains drafts and finished outputs.
- Module-level editability is a core feature, not a nice-to-have.
- HTML artifacts must be sandboxed V3-owned render templates. Do not display raw provider/model HTML with scripts, remote assets, secrets, queue credentials, or direct database actions.
- Account/auth work must never store raw local keys, OTP codes, provider tokens, or session cookie values.
- Codex Host must not run on this developer workstation for real execution. Real execution validation is only for the jump host or later Mac host.
- OpenClaw is optional and should not distract from the product mainline unless a concrete bug appears.

## Current Baseline

Completed and preserved:

- Original-assistant-style web shell with left dataset rail, main workspace, right output shelf, mobile support, upload entry, and one visible static-page/report action.
- Desktop information architecture is now fixed for the next UI round: the left rail only selects datasets, the floating top toolbar owns system status/login status/model proxy/page directory, the bottom composer owns send/upload/page actions, and the main workspace switches between assistant home, dataset management, data sources, members, and audit.
- Dataset/document basic management has started behind that shell: backend PATCH routes now support non-destructive dataset/document updates and archive lifecycle, while the dataset page can rename the selected dataset, rename documents, inspect parse details, and batch-archive documents without hard deletion.
- Home right panel is simplified into a Codex-like text surface: the top explains current project tasks, available actions, and static-page stages; the middle/lower area is a clickable generated-result index. Static-page generation workspace now keeps only the draggable module framework and module editors; natural-language revisions stay in the global bottom chat composer.
- Static-page generation uses a single main-chat progression button: `效果图——生成页面`. After module editing, the action sends the current draft to the Cloudflare/Codex image queue; once the effect image is ready, the workspace returns to chat for customer confirmation. If the customer is not satisfied, they return to module editing and repeat; if satisfied, the same button confirms the effect image and starts final page rendering.
- Static-page intent no longer interrupts ordinary conversation. When the user asks for a static page in chat, V3 still lets the assistant answer normally and appends a draft entry card at the end of the chat with `进入静态页工作台`; the main workspace switches to module editing only after the user clicks that entry.
- Upload saves real local files, classifies them, registers documents, and starts ingest workflows.
- Ingest supports local text, OOXML, PDF, OCR fallback, MiniMax document VLM slice, and first audio/video metadata/transcript slice.
- Retrieval and static-page data snapshots preserve media timestamp windows, source locators, and evidence refs for transcript/scene/keyframe citations.
- Dataset visibility foundation, local-key binding, public/private filtering, and selected-scope semantics exist.
- AssistantRun persistence exists for ordinary chat, hidden conversation memory, scope planning, evidence supply, continuation, static-page drafts, and artifacts.
- Account/email auth first pass and second-round access hardening exist across datasets, documents, AssistantRun, memory, workflows, reports, static pages, outputs, and audit.
- Static-page runtime supports deterministic and provider-backed intent interpretation with sanitized operations.
- Cloudflare/Codex image queue integration exists for static-page preview jobs.
- Static-page renderer can render layout-aware core module types into HTML/SVG and preserve safe ECharts JSON hydration islands with deterministic offline fallback.
- Frontend supports draft planning, image preview, final render display, durable right shelf, mobile builder, per-module micro-adjustment editors, and inline chart previews for editable module data.
- Final static-page handoff can download a real ZIP package with HTML, renderer manifest, data snapshot, module plan, runtime requirements, and README.
- Static-page render/export manifests classify module data quality as confirmed, partial, or missing; the handoff README, main workspace, and right shelf expose those counts.
- Background static-page render now preserves queued/rendering/failed/cancelled workflow state in the output and manifest so the main workspace and right shelf stay consistent.
- Assistant startup briefing and scope planning expose product capability, controlled action policy, quality-first context budget, recommended tool actions, and minimal UI intent chips.
- Java 8 parity audit is complete: Java/Vue used `gridstack` plus `echarts`; V3 keeps `react-grid-layout` and adds ECharts as advanced chart runtime.
- Model gateway seed exists in `llm-gateway` with model lanes, provider error redaction, and MiniMax reasoning block cleanup.
- Codex Host safe bridge exists in dry-run/plan-only form. Real execution is still blocked by default.

## Architecture Modules

Use these six modules for all future design and implementation decisions.

### 1. Experience UI

Owns assistant shell, main workspace, dataset display, mobile interactions, module editing UI, static-page/report views, and right shelf.

Also owns safe artifact viewers for static-page handoff previews, Codex execution reports, code review/project inventory summaries, and lightweight HTML editors that emit JSON patch requests back to V3.

Does not own model routing, dataset visibility decisions, direct queue access, database access, Codex Host flags, or execution of arbitrary HTML/JavaScript from providers.

### 2. V3 Control Plane

Owns `platform-api`, user/session/local-key semantics, dataset/document visibility, AssistantRun state, draft/output ownership, workflow submission, runtime inspect, and artifact ownership.

Does not own provider-specific HTTP details or local host execution.

### 3. Integration Plane

Owns `llm-gateway`, future `model-proxy` facade if needed, `tool-registry`, `mcp-gateway`, and prompt policy.

Do not create a separate model-proxy service until at least two independent processes need the same provider facade.

### 4. Workflow / Worker Plane

Owns explicit workflow definitions, queue claiming, ingest/retrieval/memory/report/static-page/media workers, retry/cancel/dead-letter/replay semantics, and background task status.

Do not move routine parsing, retrieval, rendering, or memory refresh into Codex Host.

### 5. External Execution Host

Owns `codex-host-agent`, host profile validation, isolated task workspace, guarded `codex exec`, redacted logs, returned artifacts, and workflow completion.

It is not a browser API, model gateway, memory authority, permission authority, or V3 scheduler.

### 6. Data / Artifact Plane

Owns PostgreSQL, object storage assets, vector indexes, analytical files, published static-page/report artifacts, migrations, and backup/restore discipline.

Also owns durable HTML artifact records, manifests, provenance, versioning, and export packages. HTML artifacts are products of trusted V3 templates plus sanitized data, not arbitrary browser documents.

Fresh production-like environments target PostgreSQL 17.9.

## Roadmap Priority

### Priority 1: Static-Page Product Mainline

This is the next active development direction.

The product must support:

- Model-driven static-page planning in the main workspace.
- Module layout with `{ x, y, w, h }` on desktop.
- Mobile vertical reorder only.
- Direct module title/content/data/chart edits.
- Natural-language module changes through AssistantRun/ReAct operations.
- Data snapshot handoff from retrieval/user edits to image prompt and final render.
- Cloudflare/Codex preview image queue.
- Effect image confirmation.
- GPT Image 2 or equivalent image providers generate the visual effect preview and visual contract only; they do not produce the final editable/static HTML page.
- Final static-page render/export with real data and deterministic fallback.
- ECharts advanced chart runtime for Java 8 parity and complex dashboards.

### Priority 2: Assistant/RAG/Ingest Quality

Continue improving supply quality, not UI form complexity:

- Better scope planner candidates.
- More aggressive RAG/detail supply when selected or inferred datasets match.
- Hidden conversation memory retrieval only when useful.
- Better document structured profiles from MiniMax VLM metadata.
- Media detail API for transcript windows, scenes, keyframes, OCR snippets, and provider evidence.
- No foreground parsing beyond save/preclassify/register/enqueue.

### Priority 3: Safe HTML Artifact Layer

This is a shared product surface, not a replacement for the static-page renderer.

Use the idea behind [HTML Effectiveness](https://thariqs.github.io/html-effectiveness/#code-review) as a pattern reference: self-contained HTML can be an excellent review/report medium when it is generated from trusted templates, sandboxed, and paired with structured data.

V3 should use this layer for:

- Codex Host execution reports and step summaries.
- Static-page planning handoff pages that show module layout, evidence, missing-data warnings, and next actions.
- Code review/project inventory pages for internal development workflows.
- Lightweight temporary editors that collect edits and send JSON patch operations back to V3.

Do not use this layer for:

- Raw model-generated browser apps.
- Remote scripts, remote CSS, tracking pixels, provider tokens, or direct database/queue operations.
- Customer-facing final static-page delivery when the deterministic static-page renderer is the correct product artifact.

### Priority 4: Account/Auth Maintenance Only

Account work is currently paused after second-round hardening.

Only fix:

- Access leaks.
- OTP/security bugs.
- Session/cookie breakage.
- Migration/runtime blockers.

Do not start team sharing, robot ownership, admin audit UI, or deep encryption recovery until product mainline is stable.

### Priority 5: Codex Host / Model Proxy

Continue as a sidecar execution-kernel track, not as the product mainline.

Current status:

- `codex_host_task` exists and is disabled by default.
- Workflow queue and `codex-host-agent` dry-run/plan-only exist.
- Real execution is not validated and must not run on this workstation.
- `llm-gateway` has model lanes and MiniMax-compatible routing foundations.

Next only after product mainline slices are not blocked:

- Shared Codex Host contracts outside `platform-api`.
- Dependency cleanup for `codex-host-agent -> platform-api`.
- First-class task memory space creation.
- Jump-host/Mac-host real execution validation.
- Internal model-proxy facade only when `llm-gateway` extraction is justified.

### Priority 6: OpenClaw Frozen

OpenClaw optional provider/stubs completed their first useful pass.

Do not continue OpenClaw as a main execution-kernel route.

## Overall Architecture Review

The project should stay split into two tracks:

- Product track: assistant context supply, dataset/RAG/media quality, static-page planning/editing, preview image, final render, export package, and right-shelf artifact lifecycle.
- Execution extension track: Codex Host, model proxy, task memory spaces, and HTML execution reports.

The product track must remain shippable without Codex Host. Codex Host can improve execution depth and local automation later, but V3 must still answer, retrieve, generate reports/static pages, and manage artifacts through its own API/worker/data planes.

The next highest leverage order is:

1. Finish static-page module editability and final-render/export quality.
2. Finish AssistantRun context supply so the model understands selected/inferred datasets, current draft state, and available report/static-page actions.
3. Add the safe HTML artifact viewer as a common review/report surface, starting with Codex Host reports and static-page planning handoffs.
4. Validate real Codex Host execution only on the jump host or later Mac host.
5. Resume account expansion only when product workflows need it.

The main architectural risk is letting three "builders" compete: static-page renderer, HTML artifact renderer, and Codex Host. The boundary is strict: static-page renderer produces customer report pages, HTML artifact renderer displays review/control artifacts, and Codex Host executes external tasks but does not own V3 product state.

## Immediate Execution Track

The next development thread should continue with static-page final-render/chart/data quality.

### Task 0: Make This Master Plan The Entry Point

**Files:**

- Create: `docs/plans/2026-05-07-v3-master-development-plan.md`
- Modify: `docs/plans/2026-04-27-static-page-generation-studio-plan.md`
- Modify: `docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md`

**Steps:**

1. Add this master plan.
2. Add a top note in older detailed plans pointing to this master plan.
3. Run `git diff --check`.
4. Commit with `docs: add v3 master development plan`.

**Acceptance:**

- A fresh thread can find the active plan in under one minute.
- Older plans are clearly marked as source references.

### Task 1: Add ECharts Runtime Dependency And Contract

**Files:**

- Modify: `apps/web/package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/lib/static-page-draft.test.mjs`
- Create: `apps/web/app/lib/static-page-chart-runtime.js`
- Test: `apps/web/app/lib/static-page-draft.test.mjs`

**Steps:**

1. Add `echarts` with `pnpm --filter @ai-data-platform-v3/web add echarts`.
2. Add chart runtime fields to the draft normalization path: `deterministic` and `echarts`.
3. Sanitize ECharts module options to allow only supported chart types and plain JSON data/options.
4. Reject functions, raw HTML, script-like strings, remote URLs, and unknown runtime values.
5. Add tests for default runtime, ECharts runtime, invalid runtime fallback, and unsafe option stripping.
6. Run `node --test apps/web/app/lib/static-page-draft.test.mjs`.
7. Run `Push-Location apps/web; npm run build; Pop-Location`.

**Acceptance:**

- Existing drafts without runtime keep working.
- Advanced modules can request ECharts safely.
- Unsafe chart options do not reach the renderer.

### Task 2: Add Frontend ECharts Preview Adapter

**Files:**

- Create: `apps/web/app/components/static-page/StaticPageChartPreview.js`
- Modify: `apps/web/app/components/static-page/StaticPageFinalRender.js`
- Modify: `apps/web/app/components/static-page/StaticPageModuleCard.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Create a client-only chart preview component for ECharts modules.
2. Keep deterministic module rendering for basic chart modules.
3. Render ECharts only from sanitized JSON options and current module data snapshot.
4. Add compact chart-runtime labels in module cards so the user can see whether a module is basic or advanced.
5. Keep mobile rendering lightweight; no complex editor grid on mobile.
6. Run `Push-Location apps/web; npm run build; Pop-Location`.

**Acceptance:**

- Static-page planning remains usable without a dataset.
- ECharts preview works only when module data exists.
- Missing data shows explicit partial/missing state, not fake bars.

### Task 3: Add Backend Chart Runtime And Snapshot Compatibility

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/static-page-runtime/src/lib.rs`
- Modify: `crates/static-page-renderer/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: relevant unit tests in the same crates

**Steps:**

1. Extend static-page module contracts with chart runtime and sanitized chart options.
2. Ensure provider-backed intent can request ECharts but cannot send executable code.
3. Keep `dataSnapshot` as the shared source for image prompt and final render.
4. Add renderer manifest fields that record chart runtime, data quality, and fallback mode.
5. Keep deterministic HTML/SVG output as fallback for export.
6. Run `cargo fmt --all --check`.
7. Run `cargo test -p static-page-runtime`.
8. Run `cargo test -p static-page-renderer`.
9. Run targeted `cargo test -p platform-api static_page_data_snapshot`.

**Acceptance:**

- Image prompt payload and final render manifest use the same data snapshot.
- ECharts runtime does not bypass no-fake-data rules.
- Final render can still complete without browser-side ECharts.

### Task 4: Harden Module Editing As The Core Sell Point

**Files:**

- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/lib/static-page-draft.test.mjs`
- Modify: `crates/static-page-runtime/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Current implementation note:** Backend draft operations now refresh the design contract even when the client sends a full edited payload. Confirmed/queued/running preview contracts are marked `stale` when the module/layout/render/mobile-order fingerprint changes, stale edits clear `previewImage` and `finalPage`, and ReAct/current-draft operations continue to apply against the persisted current draft before recording operation metadata. The web draft helper now uses the same stale rules, keeps a stale queued job visible as needing regeneration, exposes effect/final status in the planning summary, and styles the stale card correctly inside the dark assistant shell. Desktop and mobile builders now keep only module layout/editing controls; model-led whole-draft changes go through the global bottom chat composer. Static-page creation requests from chat are non-interruptive: the assistant still answers normally, and the draft entry card is appended after the messages with `进入静态页工作台`. The main chat card owns the single `效果图——生成页面` CTA after the user enters/edits the draft; it first queues/refreshes the effect image and then, after customer approval, confirms the visual contract and starts final rendering. The effect-image ready state closes the module editor and returns the user to chat; dissatisfied users reopen module editing from the same card.

**Steps:**

1. Ensure every module can edit title, text, data rows, data binding, visualization type, chart runtime, and layout.
2. Keep direct editing lightweight; natural-language update remains the primary path.
3. Make stale preview/final render state obvious after module edits.
4. Ensure model-requested operations only mutate the current visible draft.
5. Add tests for text edits, data row edits, visualization switch, ECharts runtime switch, layout resize, and stale artifact markers.
6. Run frontend and backend targeted tests.

**Acceptance:**

- Users can adjust module content without abandoning the AI workflow.
- Model operations cannot mutate another draft or hidden user's artifact.
- Every edit that changes visual/data output marks preview/final render stale.

### Task 5: Complete Background Final Render And Export Package

**Files:**

- Modify: `crates/static-page-worker/src/lib.rs`
- Modify: `crates/static-page-worker/src/main.rs`
- Modify: `crates/static-page-renderer/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/components/static-page/StaticPageFinalRender.js`
- Modify: `apps/web/app/components/InsightPanel.js`

**Current implementation note:** Web UI background render submission, main-workspace status card, right-shelf cancel/retry actions, ZIP handoff, data-quality chips, workflow queued/rendering state synchronization, worker manifest failure diagnostics, worker-side cancellation race protection, PostgreSQL-backed worker completion/cancel tests, static-page render retry/dead-letter workflow regression coverage, and API retry/dead-letter output-state coverage are implemented. Final render manifests and ZIP handoff now also include module-level data quality/runtime diagnostics via `data-quality-report.json`, so handoff reviewers can see each module's title, data binding, sample row count, quality status, fallback mode, and recommended action. The final-render panel shows a compact module-level quality list with localized status/runtime labels, the right-side static-page shelf surfaces the first problem/fallback modules directly on each draft card, and the safe HTML artifact layer now synthesizes read-only `static_page_data_quality_report` artifacts from rendered final-page manifests for review in the common artifact shelf.

**Follow-up hardening note:** Frontend draft reset now clears stale preview/final-render artifacts instead of leaving a previously rendered page attached after effect-preview reset. The backend final-render route now rejects confirmed image jobs whose preview contract fingerprint no longer matches the current draft, so stale effect images cannot be reused to produce a new final page. The web final-render panel and action dispatcher now use the same current-confirmed-preview gate before showing or accepting request/retry actions, while still allowing failed/cancelled final renders to restart when the confirmed preview is current. The right-shelf output list surfaces stale drafts as "规划已变更", hides obsolete download/retry actions, and explains that the user must regenerate and reconfirm the effect image. ReAct first-run and continue-run prompts now warn the model not to call `render_static_page` when `previewStale=true` or `previewStatus=stale`, and stale render attempts return a structured rejected observation that points the model to `submit_static_page_image_preview` instead of repeatedly trying final render.

**Visual contract package note:** Final render/export packages now include `visual-bridge.json` alongside `data-quality-report.json`. This file records the GPT Image 2/Cloudflare preview bridge, image job id, preview asset, draft fingerprint, stale reason, style direction, render model, and final HTML source so reviewers can verify that the effect image was used only as a visual contract and not as the final editable/static page.

**Steps:**

1. Use background render mode from the web UI.
2. Show queued/rendering/completed/failed/cancelled states in main workspace and right shelf.
3. Add retry/cancel controls using existing workflow signal routes.
4. Package rendered HTML, manifest, data snapshot, runtime notes, and assets into a downloadable ZIP artifact.
5. Preserve draft/output owner checks.
6. Run `cargo test -p static-page-worker`.
7. Run `cargo test -p workflow-definitions`.
8. Run `Push-Location apps/web; npm run build; Pop-Location`.

**Acceptance:**

- Final static page is durable after refresh.
- User can continue chatting after opening a draft or output.
- Exported package includes enough manifest data to debug chart/data/runtime decisions.
- Downloaded package is an actual ZIP, not only a JSON descriptor.

### Task 6: Improve Assistant Context Supply For Static Pages

**Status:** In progress. Frontend startup briefing and scope planner expose static-page/report/media capabilities, controlled continuous-action policy, recommended tool actions, selected/inferred visible-scope rules, quality-first context budget, stale static-page preview/export blockers, and compact UI intent/action chips. Backend AssistantRun scope planning now emits the same supply-policy contract, enriches visible dataset candidates from real visible documents/chunks, carries recommended tool actions into context/evidence state, preserves ReAct protocol action names separately, exposes current static-page artifact status/module count/preview stale/final-render state in the weak ReAct planning catalog without leaking module body content, summarizes the currently opened static-page artifact in provider prompts as an operable skeleton of draft id/module ids/titles/layout/data-binding/chart type instead of raw module body/data rows, promotes vague follow-up prompts such as "继续刚才那版改一下" to active static-page draft context when a draft is open while leaving unrelated ordinary chat alone, lets ReAct explicitly recall hidden local-thread conversation memory even when the original ordinary-chat scope had an empty `conversation_memory` array, expands detail-first evidence limits for static-page/report/media scopes, falls back to visible document chunks when selected-scope retrieval evidence has not been generated yet, adds model-facing supply briefs so provider prompts distinguish citable supplied items from detail targets, aligns retrieval-worker indexing and AssistantRun query scoring on boosted CJK phrase n-grams up to 6 characters so business phrases such as "订单延期风险" and "客户满意度" survive lexical signatures, keeps regression coverage for CJK phrase weighting plus fallback chunk ranking, and guards ordinary chat so visible datasets do not force supply. Retrieval quality still needs deeper validation on larger real customer corpora, but the local lexical baseline is now covered by realistic order/support/FAQ phrase fixtures.

**Files:**

- Modify: `crates/assistant-runtime/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/retrieval-worker/src/lib.rs`
- Modify: `apps/web/app/lib/scope-planner.js`
- Modify: `apps/web/app/lib/assistant-startup-briefing.js`

**Steps:**

1. Keep no-dataset chat as ordinary model chat.
2. Give the model concise system/database/product briefing at chat start.
3. Improve selected/inferred dataset candidate summaries.
4. Prefer full-text retrieval/details within the selected scope when static-page/report intent is detected.
5. Include hidden conversation memory only when intent requires it.
6. Add tests for selected-scope precedence, no-dataset behavior, hidden-memory inclusion, and static-page evidence supply.

**Acceptance:**

- The model knows V3 can create reports/static pages.
- Dataset candidates are visible as small UI intent notes, but final answer remains model-authored.
- Static-page drafts are better grounded without forcing the user into a form.

### Task 7: Media And MiniMax Follow-Up

**Status:** Completed in current baseline. MiniMax media support is probe-gated, media parse metadata exposes transcript/scene/keyframe evidence, media detail APIs exist, and retrieval/static-page evidence now preserves timestamp citations without assuming missing transcripts.

**Files:**

- Modify: `crates/ingest-worker/src/lib.rs`
- Modify: `crates/document-vlm-runtime/src/lib.rs`
- Create or modify: `crates/media-worker/src/lib.rs` only if a separate worker becomes necessary
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Keep upload foreground non-blocking.
2. Add exact MiniMax capability probes for audio/video parsing before marking provider support.
3. Add transcript segment and keyframe/scene metadata when local tools provide it.
4. Add media detail API for transcript windows, scene windows, OCR snippets, and provider evidence.
5. Feed media evidence into retrieval/report/static-page supply only through visible documents.
6. Do not include gated/authorization-required OSS models by default.

**Acceptance:**

- Audio/video uploads are first-class materials.
- Missing transcription becomes partial parse, not hallucinated transcript.
- Static-page/report flows can cite media evidence by timestamp when available.

### Task 8: Codex Host Contract Cleanup

**Status:** Completed in current baseline. Shared request/result contracts live in `contracts`, `codex-host-agent` no longer depends on `platform-api`, dry-run/plan-only remain safe defaults, `codex_exec` is still host/profile gated, task memory space ids are first-class in the queue context, and successful worker outputs now serialize through `CodexHostTaskOutputView` with mode-specific AssistantRun events. Real `codex_exec` now additionally requires `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT`; the agent creates a task-scoped workspace label from `task_memory_space_id` and runs Codex from that directory instead of the host agent's current working directory.

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/workflow-definitions/src/lib.rs`
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `docs/architecture/codex-host-bridge-contract.md`
- Modify: `docs/operations/codex-host-model-profiles.md`

**Steps:**

1. Extract shared Codex Host task request/result contracts out of `platform-api`.
2. Make `codex-host-agent` depend on contracts/workflow/storage/client code, not the full API crate.
3. Keep dry-run and plan-only as default modes.
4. Keep real `codex_exec` disabled unless host kind and profile allowlist pass.
5. Add first-class task memory space id once memory-space storage is ready.
6. Validate real execution only on jump host or later Mac host.

**Acceptance:**

- Codex Host remains an external worker.
- Browser traffic still goes only through V3 APIs.
- No local workstation Codex execution is triggered.

### Task 9: Add Safe HTML Artifact Renderer

**Status:** In progress. Frontend/contract/backend listing, durable manifest storage, and action-submission slices are implemented: web-side manifest normalization rejects unsafe templates, scripts, remote URLs, event handlers, form posts, provider tokens, queue secrets, and secret-like payload strings; the new `HtmlArtifactViewer` renders trusted templates in sandboxed iframes; the right shelf can list/open HTML artifacts; shared Rust contracts now include `HtmlArtifactManifestView`; Codex Host dry-run/plan-only/codex-exec outputs can attach a read-only `codex_execution_report` manifest without embedding raw process logs; storage has a standalone `html_artifacts` table/repository; `/v1/html-artifacts` now prefers persisted artifacts by run id or local thread id and opportunistically backfills older AssistantRun event manifests; backend listing synthesizes interactive `static_page_planning_handoff` artifacts for visible static-page drafts and read-only `static_page_data_quality_report` artifacts from rendered final-page manifests; `/v1/html-artifacts?report_plan_id=...` now validates report-plan visibility, synthesizes and persists read-only `report_render_summary` artifacts for report render outputs, and the web right shelf also keeps a local fallback summary for the selected report plan's render outputs; `/v1/html-artifacts/{artifact_id}/events` accepts only V3-validated `html_artifact.patch` or `html_artifact.action_intent` submissions for non-read-only backend artifacts and records them as AssistantRun events; static-page planning handoff patches with owner scope `static_page_draft` execute through a restricted JSON Patch -> static-page operation translator for module title/content/data binding/visualization/layout, `styleDirection`, and `mobileOrder`; action-intent handoffs now collect a natural-language prompt in the sandbox and apply it through the existing static-page intent interpreter; the frontend refreshes the target static-page draft after a successful artifact event. Non-static-page backend product persistence beyond report render summaries is still pending.

**GPT Image 2 integration note:** The image queue sits between planning and final render. It receives the same module/data snapshot and visual spec that the final renderer will later use, then returns an effect image for user confirmation. The confirmed image is a visual reference and a fingerprinted contract, not the source of truth. Final HTML still comes from `StaticPageDraft.modules`, `dataSnapshot`, `visualSpec`, `renderSpec`, and renderer manifests. Safe HTML artifacts show this bridge in planning handoffs so reviewers can see whether the effect image is missing, queued, stale, confirmed, or ready to drive final render.

**Current visual-bridge artifact note:** Backend static-page planning handoff artifacts now include the same GPT Image 2 bridge diagnostics as the local web fallback: image job id/status, queue position/message, preview asset key, previous asset key, draft fingerprint, stale reason, style direction, render model, and final-render status. The safe artifact template renders these diagnostics without exposing queue credentials or treating the effect image as final HTML.

**Files:**

- Create: `apps/web/app/components/artifacts/HtmlArtifactViewer.js`
- Create: `apps/web/app/lib/html-artifact-manifest.js`
- Create: `apps/web/app/lib/html-artifact-manifest.test.mjs`
- Modify: `apps/web/app/components/InsightPanel.js`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `docs/architecture/codex-host-bridge-contract.md`
- Create: `crates/storage/migrations/0007_html_artifacts.sql`

**Steps:**

1. Define an `html_artifact` manifest with artifact id, owner scope, source type, template id, data refs, provenance, created time, and allowed interaction mode.
2. Add a web manifest sanitizer that rejects inline scripts, remote URLs, event handlers, form posts, provider tokens, queue secrets, and unknown template ids.
3. Render artifacts in a sandboxed iframe with no same-origin privilege by default.
4. Add first templates for `codex_execution_report`, `static_page_planning_handoff`, `static_page_data_quality_report`, `report_render_summary`, and `code_review_summary`.
5. Allow interactive templates to emit only structured JSON patch or action-intent events back to V3.
6. Add right-shelf open/view support without replacing the main static-page final renderer.
7. Add Codex Host plan-only/dry-run report output that can attach an HTML artifact manifest.
8. Persist trusted manifests in `html_artifacts` while keeping AssistantRun events as compatibility/fallback history.
9. Execute static-page planning handoff JSON patches only through V3 static-page operations, not direct JSON/database mutation.
10. Synthesize backend static-page planning handoff artifacts and execute natural-language `action_intent` through the static-page intent interpreter.
11. Synthesize read-only static-page final-render data-quality report artifacts from renderer manifests.
12. Run `node --test apps/web/app/lib/html-artifact-manifest.test.mjs`.
13. Run `Push-Location apps/web; npm run build; Pop-Location`.
14. Run `cargo test -p contracts html_artifact`.
15. Run `cargo test -p platform-api html_artifact`.

**Acceptance:**

- HTML artifacts are useful for review/report workflows without becoming arbitrary browser apps.
- Static-page final delivery still uses the deterministic static-page renderer/export package.
- Codex Host can return a readable execution report without exposing secrets or needing local workstation execution.
- Any user edit from an HTML artifact is converted into V3-validated JSON patch/action intent, never direct DOM/database mutation.

## Verification Commands

Frontend:

```powershell
node --test apps/web/app/lib/static-page-draft.test.mjs apps/web/app/lib/assistant-startup-briefing.test.mjs apps/web/app/lib/scope-planner.test.mjs apps/web/app/lib/upload-classifier.test.mjs
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
Push-Location apps/web
npm run build
Pop-Location
```

Rust targeted:

```powershell
cargo fmt --all --check
cargo test -p static-page-runtime
cargo test -p static-page-renderer
cargo test -p static-page-worker
cargo test -p retrieval-worker
cargo test -p workflow-definitions
cargo test -p platform-api static_page_data_snapshot
cargo test -p platform-api assistant_run_react_static_page
cargo test -p platform-api assistant_run
cargo test -p contracts html_artifact
cargo test -p platform-api html_artifact
```

Account/security maintenance:

```powershell
cargo test -p contracts --lib auth
cargo test -p storage --lib
cargo test -p platform-api --lib auth_audit_events_endpoint_returns_current_user_redacted_events
cargo test -p platform-api --lib email_auth_verify_rejects_non_login_purpose_for_session_creation
```

Codex Host:

```powershell
cargo test -p platform-api codex_host
cargo test -p workflow-definitions codex_host
cargo test -p codex-host-agent
cargo check -p codex-host-agent
```

## Commit Discipline

- Commit plan-only changes separately.
- Commit static-page ECharts dependency separately from renderer/export work.
- Commit frontend module editing separately from backend contract work when possible.
- Commit safe HTML artifact renderer separately from static-page final-render delivery.
- Commit Codex Host contract cleanup separately from real host validation.
- Never commit `.storage`, generated keys, provider tokens, local env files, queue credentials, or smoke-test artifacts.
- Run `git diff --check` before each commit.

## Fresh Thread Prompt

Use this prompt for the next development thread:

```text
Continue AI Data Platform V3 from docs/plans/2026-05-07-v3-master-development-plan.md.
Treat that file as the active master plan; older plans are source references only.
Product mainline is static-page generation: module editing, data snapshots, ECharts advanced runtime, Cloudflare/Codex preview, and durable final render/export.
Safe HTML artifacts are a shared review/control surface for Codex reports, planning handoffs, and lightweight JSON-patch editors; do not confuse them with final customer static-page delivery.
Do not resume account expansion unless fixing a security/access regression.
Do not run real Codex execution on the local developer workstation; Codex Host real validation is only for the jump host or later Mac host.
Continue with the next static-page final-render/chart/data-quality slice unless a Codex Host validation blocker is explicitly requested.
```
