# AI Data Platform V3 Thread Handoff

**Date:** 2026-05-12

**Repository:** `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3`

**Branch:** `main`

**Latest pushed commit:** `14c09fa Render video artifact group audit`

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
- Video PPT/transcript extraction pipeline is partially present but now has a much stronger deliverable lifecycle: direct/public video source resolution, workflow stubs, media worker stages, ingest worker media parsing, raw frame/contact sheet/selected slide/final manifest artifacts, basic screenshot PPTX output, `slide_notes.md`, `subtitle_page_map.json`, grouped durable output artifacts, read-only video summary artifacts, guarded downloads, completion follow-up actions, status-only user notification metadata, and redacted completion audit rows with artifact group counts.

Most recent development slice:

- Static-page data snapshot now adds per-module `bindingQuality`, `bindingQualityStatus`, `chartDataFit`, and `recommendedAction`.
- Static-page runtime provider input receives compact binding-quality summaries without leaking raw sample rows.
- ReAct current-artifact briefs expose quality-only binding summaries.
- Codex plan-only action suggestions now consume static-page `bindingQuality`: if module/chart data still needs attention, Codex suggests module repair or retrieval instead of submitting effect-image preview; confirmed quality still allows the normal preview path.
- Actual backend image-preview and final-render routes also block weak `bindingQuality` submissions and return structured gate details. The frontend preserves those details and avoids optimistic render/preview state when the backend rejects a weak draft.
- Video/PPT deliverables now flow through durable `video_extraction_artifacts` with `manifest_outputs`, `final_outputs`, `review_outputs`, and `evidence_outputs`; the safe HTML summary and right shelf expose ready states, next actions, and redacted audit counts without host-composing the final assistant answer.

## Important Current Gap

The static-page preview/final-render data-quality gate is now wired into the real backend path as well as Codex plan-only suggestions. The current highest-value gap is no longer "can the button bypass the gate"; it is proving the video/PPT flow end-to-end with a controlled sample and jump-host validation.

Recommended next small closure:

1. Pick or create a controlled video fixture that can safely run through the existing media-worker path.
2. Run the local deterministic `media-worker` tests first, then use the jump host for any Codex/real-video smoke work; do not run real `codex exec` locally.
3. Verify the resulting summary exposes `PPTX`, `final_deliverables_manifest`, `extraction_artifacts_manifest`, `slide_notes`, `subtitle_page_map`, grouped output lists, guarded downloads, completion follow-up, and redacted audit counts.
4. Fix only the first concrete gap found in that end-to-end path; keep login-gated acquisition, QR login, cookies, and recording bypass out of scope.

## Suggested Next Work Order

1. **Video/PPT end-to-end validation**
   - Run a controlled video/PPT extraction path and verify deliverable files, right-shelf discovery, guarded downloads, safe HTML summary, completion follow-up, and redacted audit group counts.
   - Keep login-gated acquisition out of scope.
   - Likely files: `crates/media-worker`, `crates/ingest-worker`, `crates/platform-api/src/lib.rs`, `apps/web/app/lib/html-artifact-manifest.js`, web right shelf code, and fixtures/smoke docs if needed.

2. **Video/PPT quality enhancement**
   - Improve rectangle extraction/dedupe promotion, richer PPTX speaker notes, subtitle-to-page correction, and low-confidence warnings.
   - Use the local `wechat-video-ppt-extract` skill only as the fixed post-video SOP: frame import/capture, contact sheet, rectangle extraction, conservative dedupe, keep-list, screenshot PPTX with manifest/source notes.

3. **Static-page planning quality**
   - Improve natural-language module planning, data binding selection, chart type selection, and evidence-to-module mapping.
   - Strengthen "do not fake data" behavior for image prompt generation and final render.

4. **Codex executor bridge**
   - Keep direct execution off this machine.
   - Continue dry-run/plan-only parity first.
   - Then jump-host shadow smoke.
   - Then small traffic plan-only comparison.
   - Only after that consider real execution gates.

5. **Static-page gate maintenance**
   - Treat preview/final-render `bindingQuality` gates as implemented baseline.
   - Only revisit if a direct UI/API bypass, unclear user-facing repair hint, or regression appears.

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

Current latest pushed commit should be 14c09fa: "Render video artifact group audit".

Keep the current UI shell stable. Do not redesign the assistant layout. Focus on backend capability and quality.

Start with the next recommended small closure: validate the video/PPT extraction deliverable path end-to-end with a controlled sample. Confirm that the workflow exposes PPTX, final and extraction manifests, slide notes, subtitle page maps, grouped durable outputs, guarded downloads, completion follow-up metadata, and redacted audit group counts. Use the jump host for any real Codex validation; do not run real `codex exec` locally.

Do not run real Codex execution on this local machine. Do not delete files directly. Run focused tests and commit only verified, low-risk changes.
```
