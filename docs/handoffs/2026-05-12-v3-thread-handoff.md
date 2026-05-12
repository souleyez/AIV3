# AI Data Platform V3 Thread Handoff

**Date:** 2026-05-12

**Repository:** `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3`

**Branch:** `main`

**Latest pushed commit:** `cefbeb7 Gate static page previews on data quality`

**Active master plan:** `docs/plans/2026-05-07-v3-master-development-plan.md`

This handoff is for continuing V3 development in a fresh Codex thread. Treat the master plan as the source of truth. Older plan files are references only.

## Current Product Direction

V3 is a data-aware intelligent assistant. The visible UI shell is considered stable. Do not redesign the left dataset rail, top toolbar, home-only composer, right output shelf, static-page entry card, directory pages, or the single `效果图——生成页面` CTA unless fixing a concrete blocking bug.

Current priority is backend/product capability quality:

- Static-page generation quality: planning, module data binding, image-preview prompts, final render fidelity, export diagnostics.
- Assistant/RAG quality: intent scope, selected/inferred datasets, hidden conversation memory, evidence supply, detail retrieval.
- Model gateway and Codex conversation executor: Codex becomes the reasoning/action kernel behind V3 validation, not a replacement for V3 control.
- Media/video PPT extraction: support direct upload, direct video URLs, and public pages where V3 can safely resolve video assets. Login-gated pages, QR login, cookies, private hosts, and recording bypass are out of scope.

## Non-Negotiable Rules

- No fake data. Missing or partial evidence must stay visible to the model, UI, renderer, and export package.
- No host-composed assistant answer. V3 supplies context, evidence, and action contracts; the model writes final text.
- V3 remains the authority for dataset visibility, memory scope, workflow state, queue submission, action validation, and artifact ownership.
- Dataset selection is supply preference, not a separate chat.
- Conversation history is a hidden dataset and enters context only through intent/scope policy.
- Foreground upload must only save, preclassify, register, and enqueue. Heavy parsing stays in background workers.
- Do not run real Codex execution on this local developer workstation. Real Codex validation belongs on the jump host or later Mac host.
- Do not directly delete files in this workspace. If cleanup is needed, use the backup-first helper required by `AGENTS.md`.
- Never commit `.storage`, keys, provider tokens, env files, queue credentials, or local smoke-test artifacts.

## Current Baseline

Completed and pushed:

- UI shell is close to fixed: home keeps the composer; dataset/data-source/member/audit pages do not keep chat input; mobile has dataset/output drawer ideas already reflected in UI work; members page has clear login overlay.
- Dataset/document management has backend PATCH/archive support and frontend basic rename, detail inspection, and batch archive flows.
- Static-page workspace supports draggable module planning, module editing, image preview state, final render display, durable right shelf, mobile builder, and inline chart previews for module data.
- Static-page final handoff can produce a real ZIP with HTML, renderer manifest, data snapshot, module plan, runtime requirements, and README.
- Static-page renderer supports layout-aware HTML/SVG output, deterministic fallback, ECharts hydration islands, and data quality manifest counts.
- Cloudflare/Codex image queue integration exists for effect-preview jobs.
- Assistant startup briefing, scope planning, evidence supply, continuation, hidden memory, static-page drafts, and artifacts are persisted through AssistantRun flows.
- Model gateway seed exists in `llm-gateway` with model lanes, provider error redaction, and MiniMax reasoning-block cleanup.
- Codex Host bridge exists in dry-run/plan-only form. Real execution remains blocked by default.
- Safe HTML artifacts exist for planning handoff, execution/report summaries, and controlled JSON-patch/action-intent flows.
- Video PPT/transcript extraction pipeline is partially present: direct/public video source resolution, workflow stubs, media worker stages, ingest worker media parsing, raw frame/contact sheet/selected slide/final manifest artifacts, basic screenshot PPTX output, and read-only video summary artifacts.

Most recent development slice:

- Static-page data snapshot now adds per-module `bindingQuality`, `bindingQualityStatus`, `chartDataFit`, and `recommendedAction`.
- Static-page runtime provider input receives compact binding-quality summaries without leaking raw sample rows.
- ReAct current-artifact briefs expose quality-only binding summaries.
- Codex plan-only action suggestions now consume static-page `bindingQuality`: if module/chart data still needs attention, Codex suggests module repair or retrieval instead of submitting effect-image preview; confirmed quality still allows the normal preview path.

## Important Current Gap

The latest gate is implemented in the Codex plan-only action suggestion layer. It improves model/action planning, but it is not yet guaranteed to block every direct UI/API preview submission path.

Recommended next small closure:

1. Trace the actual `效果图——生成页面` frontend/API path.
2. Add a V3-side validation or warning gate before preview queue submission when current draft `bindingQuality` reports missing bindings, inferred signals, matched-field-only candidates, or chart modules without sample rows.
3. Keep user flow simple: the user should see a concise reason and can return to module editing; avoid adding new global UI.
4. Add tests that prove direct preview submission cannot silently bypass the same quality gate that Codex plan-only now follows.

This closes the loophole between "model suggested action" and "user clicked button".

## Suggested Next Work Order

1. **Static-page preview gate hardening**
   - Make preview queue submission respect `bindingQuality` in the actual backend/API path.
   - Preserve user agency, but do not let weak data become a polished fake effect image without explicit visible warning.
   - Likely files: `crates/platform-api/src/lib.rs`, `apps/web/app/lib/static-page-draft.js`, static-page frontend action components, and existing static-page tests.

2. **Video/PPT deliverable lifecycle**
   - Ensure `final_deliverables_manifest.json` appears in durable artifact/output lists.
   - Add right-shelf discovery, download entry, and artifact status normalization.
   - Keep login-gated acquisition out of scope.
   - Likely files: `crates/media-worker`, `crates/ingest-worker`, `crates/platform-api/src/lib.rs`, artifact listing code, web right shelf code.

3. **Video/PPT quality enhancement**
   - Add Markdown transcript/output, better PPTX speaker notes, subtitle-to-page mapping, and low-confidence warnings.
   - Use the local `wechat-video-ppt-extract` skill only as the fixed post-video SOP: frame import/capture, contact sheet, rectangle extraction, conservative dedupe, keep-list, screenshot PPTX with manifest/source notes.

4. **Static-page planning quality**
   - Improve natural-language module planning, data binding selection, chart type selection, and evidence-to-module mapping.
   - Strengthen "do not fake data" behavior for image prompt generation and final render.

5. **Codex executor bridge**
   - Keep direct execution off this machine.
   - Continue dry-run/plan-only parity first.
   - Then jump-host shadow smoke.
   - Then small traffic plan-only comparison.
   - Only after that consider real execution gates.

Account/member/auth work is maintenance-only unless a security/access bug appears.

## Key Files

- Master plan: `docs/plans/2026-05-07-v3-master-development-plan.md`
- Static-page frontend draft helpers: `apps/web/app/lib/static-page-draft.js`
- Static-page runtime/provider input: `crates/static-page-runtime/src/lib.rs`
- Static-page renderer/export: `crates/static-page-renderer/src/lib.rs`
- Platform API and AssistantRun/ReAct/static-page routes: `crates/platform-api/src/lib.rs`
- Assistant runtime scope planner and Codex plan-only executor: `crates/assistant-runtime/src/lib.rs`
- Model gateway: `crates/llm-gateway/src/lib.rs`
- Codex host agent: `crates/codex-host-agent/src/main.rs`
- Media worker: `crates/media-worker/src/main.rs`
- Ingest worker: `crates/ingest-worker/src/main.rs`
- Workflow contracts: `crates/workflow-definitions/src/lib.rs`
- Shared contracts: `crates/contracts/src/lib.rs`

## Verification Commands

Run the smallest relevant set first, then expand if touched areas cross boundaries.

```powershell
cargo test -p assistant-runtime codex_executor
cargo test -p platform-api static_page
cargo test -p static-page-runtime
cargo test -p static-page-renderer
node --test apps/web/app/lib/static-page-draft.test.mjs
npm --prefix apps/web run build
git diff --check
```

For media/video changes:

```powershell
cargo test -p media-worker
cargo test -p ingest-worker
cargo test -p workflow-definitions video
cargo test -p platform-api video
```

For Codex host changes:

```powershell
cargo test -p llm-gateway
cargo test -p assistant-runtime codex
cargo test -p codex-host-agent
cargo check -p codex-host-agent
```

Do not run real `codex exec` locally.

## Fresh Thread Prompt

```text
Continue AI Data Platform V3 from C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3.

Read:
- docs/handoffs/2026-05-12-v3-thread-handoff.md
- docs/plans/2026-05-07-v3-master-development-plan.md

Current latest pushed commit should be cefbeb7: "Gate static page previews on data quality".

Keep the current UI shell stable. Do not redesign the assistant layout. Focus on backend capability and quality.

Start with the next recommended small closure: trace the actual static-page `效果图——生成页面` preview submission path and make direct preview submission respect the same `bindingQuality` data-quality gate now used by Codex plan-only action suggestions. If the current draft has missing bindings, matched-field-only candidates, inferred chart signals, or chart modules without sample rows, V3 should request retrieval/module repair or show a concise warning instead of silently queueing a polished effect image.

Do not run real Codex execution on this local machine. Do not delete files directly. Run focused tests and commit only verified, low-risk changes.
```
