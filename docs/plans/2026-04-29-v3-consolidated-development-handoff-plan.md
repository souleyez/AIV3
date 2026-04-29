# AI Data Platform V3 Consolidated Development Handoff Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Consolidate the V3 assistant, static-page generation, document/media parsing, report outputs, and optional OpenClaw extension into one execution plan that a fresh development thread can continue from without re-reading the whole project history.

**Architecture:** V3 remains a host-controlled data platform: PostgreSQL is the source of truth, the assistant supplies model context rather than composing answers locally, and all data visibility, AssistantRun state, draft state, queue state, and output artifacts are owned by V3. Static-page generation is the core product loop; OpenClaw is an optional sidecar extension for model configuration, memory augmentation, and readonly/local execution, and must be removable without changing normal V3 behavior.

**Tech Stack:** Next.js 16 / React 19 in `apps/web`; Rust crates including `platform-api`, `llm-gateway`, `static-page-runtime`, `static-page-worker`, `static-page-renderer`, `ingest-worker`, `retrieval-worker`, `memory-worker`, `document-vlm-runtime`; PostgreSQL 17.9 target; Cloudflare/Codex image queue endpoint; optional OpenClaw Gateway `/v1/responses` and `/v1/chat/completions`; local-first parsers plus configured MiniMax VLM/media capability probes.

---

## Source Plans

Use this document as the active execution entry point.

Detailed source documents remain valid as references:

- `docs/plans/2026-04-27-static-page-generation-studio-plan.md`: detailed static-page, assistant shell, ingestion, visibility, AssistantRun, and renderer plan/history.
- `docs/plans/2026-04-29-openclaw-extension-adapter-plan.md`: detailed optional OpenClaw adapter plan.
- `docs/plans/2026-04-23-v3-development-plan.md`: older high-level V3 phase baseline.
- `docs/plans/2026-04-25-static-page-visual-workbench-v3-plan.md`: older visual-workbench plan; keep only as Cloudflare image-provider and visual-contract reference. Do not revive the separate popup/workbench direction unless explicitly requested.

## Current Baseline

Latest committed code baseline:

```text
dd14693 feat(static-page): harden visual generation workflow
```

Plan files in this consolidation slice:

```text
docs/plans/2026-04-27-static-page-generation-studio-plan.md
docs/plans/2026-04-29-openclaw-extension-adapter-plan.md
docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md
```

Recent verified capabilities:

- Original-assistant-style web shell exists with left dataset rail, main chat/workspace area, right draft/output shelf, mobile support, upload entry, and one static-page generation action.
- Upload path saves real local files, classifies them, registers documents, and starts ingest workflows.
- Ingest worker has local text, OOXML, PDF, OCR fallback, MiniMax document VLM slice, and first media upload slice.
- Dataset visibility foundation exists with local key binding, public/private dataset filtering, and selected-scope semantics.
- AssistantRun persistence exists for ordinary chat, hidden conversation memory, deterministic scope planning, evidence supply, continue API, static-page draft creation, and output artifacts.
- Static-page runtime has deterministic and provider-backed intent interpretation with operation sanitization.
- Static-page image worker can call the Cloudflare/Codex queue, poll artifact status, normalize artifact URLs, and record failures.
- Static-page renderer is layout-aware and can render core module types into HTML/SVG with design contract data.
- Frontend supports static-page draft planning, image preview display, final render display, durable right shelf, and per-module micro-adjustment editors.

## Locked Product Decisions

- Ordinary chat without selected dataset behaves like normal model chat, but the model still receives a concise system/product/database briefing.
- Dataset selection means supply preference, not a hard UI mode. User-selected and model-preselected datasets follow the same selected-scope semantics.
- If no dataset is selected or inferred, do not fake data. Answer normally.
- When a relevant dataset is selected or inferred, V3 should retrieve and supply as much useful evidence as practical. Token thrift is not the first priority.
- Conversation history is a hidden dataset. It enters context only when intent/context policy says it helps.
- V3 should not locally compose the final answer from evidence. V3 supplies evidence, tools, and state; the model answers.
- Static-page planning lives in the main assistant workspace, not in a separate popup. The right panel remains drafts and finished outputs.
- Users should mostly talk naturally. UI forms are secondary affordances and should not become the main workflow.
- Module-level edits are part of the core sell point and must remain first-class.
- OpenClaw is optional. Its absence must not break startup, upload, chat, static-page deterministic flow, reports, parsing, rendering, or workers.

## Architecture Evaluation

The static-page plan and OpenClaw plan do not conflict if the boundary is kept strict.

Compatible points:

- Static-page provider-backed intent already goes through `llm-gateway::LlmProvider`; OpenClaw can slot in as another provider.
- AssistantRun ordinary chat already builds V3 evidence state before calling a provider; OpenClaw can receive the same already-filtered context.
- Hidden conversation memory exists in V3; OpenClaw memory can be an additional labeled evidence candidate.
- Continuous execution exists as AssistantRun continuation; OpenClaw local execution can be a bounded capability bridge recorded in the same trail.

Potential conflicts and decisions:

- OpenClaw memory vs V3 memory: V3 remains source of truth. OpenClaw memory is only optional evidence.
- OpenClaw local execution vs V3 host actions: V3 validates, records, and owns actions. OpenClaw may suggest or perform readonly extension tasks only through allowlisted bridge calls.
- OpenClaw model routing vs model config: OpenClaw can route model/agent choices, but runtime manifests must still record provider/model/request id in V3.
- OpenClaw availability vs production reliability: all OpenClaw lanes must be config-gated and fallback-safe.

## Workstreams

### Workstream A: Static-Page Core Product Loop

Priority: highest.

Goal: make the static-page flow feel like the actual product, not a mock.

Next outcomes:

- Module edits update content, visual type, data binding, and layout without losing draft consistency.
- Natural-language changes can update the full draft or a specific module.
- Confirmed image preview and final static page stay visually close through a stable design contract.
- Final render/export runs in background worker and produces durable artifacts.
- Right shelf lists drafts, previews, and finished outputs reliably.

### Workstream B: Assistant Intelligence And Provider Runtime

Priority: high.

Goal: make model-backed planning, answering, and continuation reliable.

Next outcomes:

- AssistantRun provider path becomes production-shaped, not just placeholder-compatible.
- Scope planning can become provider-backed while keeping deterministic fallback.
- Static-page intent provider prompt gets stricter schema and better examples.
- Provider failures are visible as runtime facts, not silent UI confusion.

### Workstream C: Optional OpenClaw Extension

Priority: high but isolated.

Goal: attach OpenClaw as optional sidecar for model config, memory augmentation, and readonly/local execution.

Next outcomes:

- `llm-gateway` has `OpenClawProvider`.
- AssistantRun and static-page intent can use `*_RUNTIME_PROVIDER=openclaw`.
- OpenClaw memory can add labeled evidence only when enabled.
- OpenClaw local execution starts disabled and readonly/allowlisted.

### Workstream D: Data Parsing, RAG, And Data Snapshots

Priority: high.

Goal: make uploaded data become useful structured evidence and chart-ready data.

Next outcomes:

- Static-page modules can bind to real dataset fields/snapshots, not only draft sample data.
- Document/image/PDF/table parsing moves closer to original project parity.
- Audio/video parsing runs as background tasks and exposes transcript/scene/keyframe evidence.
- Retrieval quality improves from lexical baseline toward richer chunk/detail selection.

### Workstream E: Reports And Output Shelf

Priority: medium-high.

Goal: make reports and static pages share the same assistant-driven creation surface and right-side artifact shelf.

Next outcomes:

- Home right shelf shows report outputs, static-page drafts, static-page renders, and finished artifacts.
- Clicking a draft/output opens it in the main workspace while keeping the composer available.
- The model knows it can initiate report/static-page creation from ordinary conversation.

### Workstream F: Security, Operations, And Deployment

Priority: continuous.

Goal: keep local-key multi-user semantics, provider secrets, and background services safe.

Next outcomes:

- Local key UX and backend visibility enforcement are consistent.
- Provider keys stay server-side or local-only as intended and never leak through UI/status.
- PostgreSQL 17.9 remains the target server version for fresh environments.
- Runtime status endpoints are redacted and useful.

## Recommended Execution Order

### Slice 0: Commit Consolidated Plans

**Files:**

- Add: `docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md`
- Add: `docs/plans/2026-04-29-openclaw-extension-adapter-plan.md`
- Modify: `docs/plans/2026-04-27-static-page-generation-studio-plan.md`

**Steps:**

1. Add consolidation notes to old source plans.
2. Run `git diff --check`.
3. Commit as `docs: consolidate v3 development handoff plan`.

**Why first:** the next thread needs one source of truth.

### Slice 1: OpenClaw Provider Adapter

**Files:**

- Modify: `crates/llm-gateway/src/lib.rs`
- Test: `crates/llm-gateway/src/lib.rs`
- Reference: `C:/Users/soulzyn/Desktop/codex/ai-data-platform/apps/api/src/lib/openclaw-adapter.ts`

**Steps:**

1. Add failing tests for `OpenClawProvider` config loading.
2. Add `OpenClawLlmProviderConfig`.
3. Add `build_provider_from_env` branch when runtime provider is `openclaw`.
4. Implement `/v1/responses` request and parser.
5. Implement chat-completions fallback.
6. Port minimal corrective retry detectors from the original adapter.
7. Run `cargo test -p llm-gateway openclaw`.

**Acceptance:**

- If `OPENCLAW_EXTENSION_ENABLED` is false or unset, no existing provider behavior changes.
- With fake OpenClaw gateway, responses and chat fallback both produce `LlmResponse`.
- Runtime manifest records provider `openclaw`.
- No token appears in errors.

### Slice 2: OpenClaw Runtime Adoption For AssistantRun And Static-Page Intent

**Files:**

- Modify if needed: `crates/platform-api/src/lib.rs`
- Modify if needed: `crates/static-page-runtime/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`
- Test: `crates/static-page-runtime/src/lib.rs`

**Steps:**

1. Add AssistantRun fake-gateway test with `ASSISTANT_RUN_RUNTIME_PROVIDER=openclaw`.
2. Add static-page intent fake-gateway test with `STATIC_PAGE_INTENT_RUNTIME_PROVIDER=openclaw`.
3. Confirm existing provider abstraction is enough; avoid OpenClaw special cases in product code.
4. Ensure static-page invalid model JSON still falls back to deterministic handling.
5. Run `cargo test -p platform-api assistant_run_openclaw`.
6. Run `cargo test -p static-page-runtime openclaw`.

**Acceptance:**

- AssistantRun evidence is built by V3 before OpenClaw receives input.
- Static-page operations are still V3-sanitized.
- OpenClaw failure does not break deterministic static-page fallback.

### Slice 3: Static-Page Module Editing And Data Binding

**Files:**

- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/lib/static-page-draft.test.mjs`
- Modify: `apps/web/app/components/static-page/StaticPageModuleCard.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningCanvas.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileModuleList.js`
- Modify: `crates/static-page-runtime/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Strengthen `update_module` operation schema for title, content, visualization, data label, data binding, and chart options.
2. Add natural-language operation examples for module-specific and global edits.
3. Add data-source selector contract based on selected-scope evidence fields.
4. Keep the current module micro-adjustment UI but make it emit the stronger operation shape.
5. Add tests for module edits, bulk wording style, visualization switch, and data binding patch.
6. Run `node --test apps/web/app/lib/static-page-draft.test.mjs`.
7. Run `cargo test -p static-page-runtime`.
8. Run web build.

**Acceptance:**

- User can adjust each module's title, text, data label, visualization type, and binding.
- Model can apply the same operation shape from natural-language prompts.
- Draft marks preview/final render stale after content or data changes.

### Slice 4: Real Static-Page Data Snapshot Path

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/retrieval-worker/src/lib.rs`
- Modify: `crates/static-page-renderer/src/lib.rs`
- Modify: `crates/static-page-runtime/src/lib.rs`
- Test: relevant crate tests

**Steps:**

1. Define a `StaticPageDataSnapshot` contract from selected datasets/documents/retrieval evidence.
2. Add backend helper to derive chart-ready values from supplied evidence and simple tables.
3. Store snapshot in draft `renderSpec` or equivalent manifest.
4. Make renderer prefer snapshot data over placeholder sample data.
5. Add tests for KPI, bar, line, donut, table, timeline, and missing-data fallback.

**Acceptance:**

- Generated static pages can use real retrieved data when available.
- Missing or weak data is explicitly labeled instead of faked.
- Renderer output and image prompt share the same data snapshot.

### Slice 5: Final Render Worker And Export Package

**Files:**

- Modify: `crates/static-page-worker/src/lib.rs`
- Modify: `crates/static-page-worker/src/main.rs`
- Modify: `crates/static-page-renderer/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/components/static-page/StaticPageFinalRender.js`

**Steps:**

1. Move final static-page render/export into background worker path.
2. Persist generated HTML, manifest, assets, and status.
3. Add retry/cancel status semantics where current image jobs already expose failures.
4. Add right-shelf durable listing for final render artifacts.
5. Add tests for queued, rendering, completed, failed, retry, and stale-after-edit states.

**Acceptance:**

- Confirmation preview can lead to a durable final static page artifact.
- User can continue chatting and editing after opening an artifact.
- Final render is not lost on page refresh.

### Slice 6: OpenClaw Memory Bridge

**Files:**

- Create: `crates/openclaw-extension/Cargo.toml`
- Create: `crates/openclaw-extension/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Create disabled-by-default memory bridge trait.
2. Add fake bridge tests.
3. Inject recalled OpenClaw memory only after V3 scope policy says memory is relevant.
4. Label evidence source as `openclaw_memory`.
5. Cap memory count with `OPENCLAW_MEMORY_LIMIT`.

**Acceptance:**

- OpenClaw memory never replaces V3 memory.
- OpenClaw memory never exposes hidden datasets.
- Runtime evidence explains when OpenClaw memory was supplied.

### Slice 7: OpenClaw Readonly Local Execution Bridge

**Files:**

- Modify: `crates/openclaw-extension/src/lib.rs`
- Create: `crates/openclaw-extension/src/execution.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add disabled-by-default execution bridge.
2. Allow only readonly first actions: `web_search`, `local_summarize`, `file_inspect_readonly`.
3. Record every bridge request and result in AssistantRun execution trail.
4. Reject write, shell, git, database, deploy, and secret access actions.
5. Add tests for disabled, allowed, rejected, and failed bridge results.

**Acceptance:**

- OpenClaw execution is useful but not dangerous.
- V3 can continue if bridge fails.
- User sees concise execution steps in the conversation/runtime trail.

### Slice 8: Parser And Media Parity

**Files:**

- Modify: `crates/ingest-worker/src/lib.rs`
- Modify: `crates/document-vlm-runtime/src/lib.rs`
- Create or modify: `crates/media-parse-worker/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/upload-classifier.js`

**Steps:**

1. Compare original parser utilities again before implementing each parser slice.
2. Add high-fidelity table recovery and structured document profiles.
3. Keep MiniMax VLM for document images/scanned PDFs/presentation pages.
4. Add audio/video transcript, scene, keyframe OCR, and provider evidence as background tasks.
5. Add media detail API for transcript windows, scene windows, OCR snippets, and timestamps.

**Acceptance:**

- Upload interaction stays fast; heavy parsing stays background.
- Parsed media evidence can be supplied to chat, reports, and static pages.
- Provider failures are recorded as recoverable parse/enrichment errors.

### Slice 9: Report And Artifact Shelf Integration

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/*`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: report runtime/worker crates as needed

**Steps:**

1. Right shelf shows report outputs, static-page drafts, preview images, and final renders.
2. Clicking any item opens it in the main workspace.
3. Composer remains available for change requests.
4. Model-facing startup briefing advertises report/static-page creation capabilities.
5. Add tests for opening draft/output from shelf and continuing edits.

**Acceptance:**

- No two-choice "chat or report" mode remains.
- Reports and static pages feel like artifacts created inside the same assistant.

### Slice 10: Security And Operations Hardening

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `apps/web/app/HomePageClient.js`
- Create or modify runtime docs

**Steps:**

1. Add redacted OpenClaw status endpoint.
2. Add provider/runtime status to operator docs.
3. Improve local key create/switch/revoke UX.
4. Ensure private/public dataset warnings are clear during upload and dataset creation.
5. Verify PostgreSQL 17.9 fresh setup path against current migrations.

**Acceptance:**

- Local multi-user model remains simple and understandable.
- Wrong-key private datasets are invisible and unusable.
- Provider secrets are never shown to the browser.

## Immediate Next Thread Instruction

Start with Slice 0 if the consolidated plan is not committed.

If Slice 0 is already committed, start with Slice 1:

```text
Implement optional OpenClawProvider in crates/llm-gateway.
Use docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md as the active plan.
Do not change static-page behavior while adding the provider.
Keep OpenClaw disabled by default and fully fallback-safe.
```

Reason:

- It is isolated and testable.
- It unlocks provider-backed AssistantRun and static-page intent without touching UI.
- It validates the optional-extension boundary before adding memory/local execution.
- It will not distract from static-page core because no runtime lane changes unless env selects `openclaw`.

After Slice 1 and Slice 2, return to Slice 3 because module editing/data binding is the static-page product core.

## Verification Commands

Frontend quick verification:

```powershell
node --test apps/web/app/lib/static-page-draft.test.mjs apps/web/app/lib/assistant-startup-briefing.test.mjs apps/web/app/lib/scope-planner.test.mjs apps/web/app/lib/upload-classifier.test.mjs
cd apps/web
npm run build
```

Rust focused verification:

```powershell
cargo test -p llm-gateway openclaw
cargo test -p static-page-runtime
cargo test -p static-page-renderer -p static-page-worker
cargo test -p platform-api assistant_run
```

Rust full verification:

```powershell
cargo fmt --check
cargo test --workspace
```

WSL equivalent when Windows Rust tooling disagrees:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test --workspace"
```

## Commit Rules

- Commit documentation consolidation separately.
- Commit provider adapter separately from runtime adoption.
- Commit static-page module editing separately from renderer/export work.
- Never commit local secret files, `.storage`, generated access keys, provider tokens, or downloaded smoke-test artifacts.
- Run `git diff --check` before every commit.

## Acceptance Criteria For This Consolidated Plan

- A fresh thread can identify the active plan in under one minute.
- OpenClaw's optional role is clear and does not threaten V3's host architecture.
- Static-page module editing remains explicitly prioritized.
- Parser/media parity remains planned as background, not foreground UI-blocking work.
- Report and static-page outputs converge into the right-side shelf and main workspace flow.
- Security and local-key visibility rules remain non-negotiable.
