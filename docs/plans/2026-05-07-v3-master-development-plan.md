# AI Data Platform V3 Master Development Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Provide the single active development plan for AI Data Platform V3, with static-page generation as the product mainline and Codex Host/model proxy as optional execution extensions behind V3 control.

**Architecture:** V3 remains the control plane and source of truth: PostgreSQL owns identity, dataset visibility, AssistantRun state, memory scope, workflow state, and artifacts. The assistant supplies model context, evidence, tools, and state but does not locally compose final answers. Static-page generation, reports, parsing, retrieval, and media understanding stay as V3 product capabilities; Codex Host is an external audited worker, not a replacement for V3's API, data plane, model gateway, or worker plane.

**Tech Stack:** Next.js 16 / React 19 in `apps/web`; `react-grid-layout` for desktop module layout; `@dnd-kit` for mobile vertical ordering; Apache ECharts for advanced chart/runtime parity; deterministic HTML/SVG rendering for export-safe fallback; Rust crates `platform-api`, `assistant-runtime`, `llm-gateway`, `static-page-runtime`, `static-page-worker`, `static-page-renderer`, `ingest-worker`, `retrieval-worker`, `memory-worker`, `document-vlm-runtime`, `codex-host-agent`; PostgreSQL 17.9 target; Cloudflare/Codex image queue; MiniMax VLM/media capability probes; OpenClaw only as optional legacy sidecar.

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
- Account/auth work must never store raw local keys, OTP codes, provider tokens, or session cookie values.
- Codex Host must not run on this developer workstation for real execution. Real execution validation is only for the jump host or later Mac host.
- OpenClaw is optional and should not distract from the product mainline unless a concrete bug appears.

## Current Baseline

Completed and preserved:

- Original-assistant-style web shell with left dataset rail, main workspace, right output shelf, mobile support, upload entry, and one visible static-page/report action.
- Upload saves real local files, classifies them, registers documents, and starts ingest workflows.
- Ingest supports local text, OOXML, PDF, OCR fallback, MiniMax document VLM slice, and first audio/video metadata/transcript slice.
- Retrieval and static-page data snapshots preserve media timestamp windows, source locators, and evidence refs for transcript/scene/keyframe citations.
- Dataset visibility foundation, local-key binding, public/private filtering, and selected-scope semantics exist.
- AssistantRun persistence exists for ordinary chat, hidden conversation memory, scope planning, evidence supply, continuation, static-page drafts, and artifacts.
- Account/email auth first pass and second-round access hardening exist across datasets, documents, AssistantRun, memory, workflows, reports, static pages, outputs, and audit.
- Static-page runtime supports deterministic and provider-backed intent interpretation with sanitized operations.
- Cloudflare/Codex image queue integration exists for static-page preview jobs.
- Static-page renderer can render layout-aware core module types into HTML/SVG.
- Frontend supports draft planning, image preview, final render display, durable right shelf, mobile builder, and per-module micro-adjustment editors.
- Java 8 parity audit is complete: Java/Vue used `gridstack` plus `echarts`; V3 keeps `react-grid-layout` and adds ECharts as advanced chart runtime.
- Model gateway seed exists in `llm-gateway` with model lanes, provider error redaction, and MiniMax reasoning block cleanup.
- Codex Host safe bridge exists in dry-run/plan-only form. Real execution is still blocked by default.

## Architecture Modules

Use these six modules for all future design and implementation decisions.

### 1. Experience UI

Owns assistant shell, main workspace, dataset display, mobile interactions, module editing UI, static-page/report views, and right shelf.

Does not own model routing, dataset visibility decisions, direct queue access, database access, or Codex Host flags.

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

### Priority 3: Account/Auth Maintenance Only

Account work is currently paused after second-round hardening.

Only fix:

- Access leaks.
- OTP/security bugs.
- Session/cookie breakage.
- Migration/runtime blockers.

Do not start team sharing, robot ownership, admin audit UI, or deep encryption recovery until product mainline is stable.

### Priority 4: Codex Host / Model Proxy

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

### Priority 5: OpenClaw Frozen

OpenClaw optional provider/stubs completed their first useful pass.

Do not continue OpenClaw as a main execution-kernel route.

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
- Modify: `apps/web/app/components/OutputShelf.js`

**Steps:**

1. Use background render mode from the web UI.
2. Show queued/rendering/completed/failed/cancelled states in main workspace and right shelf.
3. Add retry/cancel controls using existing workflow signal routes.
4. Package rendered HTML, manifest, and assets into a downloadable artifact.
5. Preserve draft/output owner checks.
6. Run `cargo test -p static-page-worker`.
7. Run `cargo test -p workflow-definitions`.
8. Run `Push-Location apps/web; npm run build; Pop-Location`.

**Acceptance:**

- Final static page is durable after refresh.
- User can continue chatting after opening a draft or output.
- Exported package includes enough manifest data to debug chart/data/runtime decisions.

### Task 6: Improve Assistant Context Supply For Static Pages

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

**Status:** Completed in current baseline. Shared request/result contracts live in `contracts`, `codex-host-agent` no longer depends on `platform-api`, dry-run/plan-only remain safe defaults, `codex_exec` is still host/profile gated, task memory space ids are first-class in the queue context, and successful worker outputs now serialize through `CodexHostTaskOutputView` with mode-specific AssistantRun events.

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

## Verification Commands

Frontend:

```powershell
node --test apps/web/app/lib/static-page-draft.test.mjs apps/web/app/lib/assistant-startup-briefing.test.mjs apps/web/app/lib/scope-planner.test.mjs apps/web/app/lib/upload-classifier.test.mjs
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
cargo test -p workflow-definitions
cargo test -p platform-api static_page_data_snapshot
cargo test -p platform-api assistant_run_react_static_page
cargo test -p platform-api assistant_run
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
- Commit Codex Host contract cleanup separately from real host validation.
- Never commit `.storage`, generated keys, provider tokens, local env files, queue credentials, or smoke-test artifacts.
- Run `git diff --check` before each commit.

## Fresh Thread Prompt

Use this prompt for the next development thread:

```text
Continue AI Data Platform V3 from docs/plans/2026-05-07-v3-master-development-plan.md.
Treat that file as the active master plan; older plans are source references only.
Product mainline is static-page generation: module editing, data snapshots, ECharts advanced runtime, Cloudflare/Codex preview, and durable final render/export.
Do not resume account expansion unless fixing a security/access regression.
Do not run real Codex execution on the local developer workstation; Codex Host real validation is only for the jump host or later Mac host.
Continue with the next static-page final-render/chart/data-quality slice unless a Codex Host validation blocker is explicitly requested.
```
