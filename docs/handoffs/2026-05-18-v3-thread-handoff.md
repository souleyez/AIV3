# AI Data Platform V3 Thread Handoff

**Date:** 2026-05-18

**Repository:** `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3`

**Branch:** `main`

**Current local HEAD:** `fc06547 Make document markdown source scrollable`

**Active master plan:** `docs/plans/2026-05-07-v3-master-development-plan.md`

This handoff is for starting a fresh Codex thread. Treat the active master plan as the source of truth. Older plans remain useful references, but when they conflict with the master plan, follow the master plan.

## Current Worktree State

At handoff creation time, the worktree contains documentation-only changes:

- `docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md`
- `docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md`
- `docs/plans/2026-05-07-v3-master-development-plan.md`
- `docs/handoffs/2026-05-18-v3-thread-handoff.md`

The plan changes freeze the Codex substrate direction and redirect the active development mainline back to provider-routed AssistantRun quality, retrieval/context quality, and static-page generation quality. No runtime code was changed in this slice.

`git diff --check` passed after the documentation updates. PowerShell/Git may print LF-to-CRLF warnings for touched Markdown files.

## Most Important Decision

The Codex substrate plan is frozen as of 2026-05-18.

Verified current state:

- 8 server has Codex CLI installed (`codex-cli 0.130.0`) but is not logged in.
- Online V3 AssistantRun still uses the provider path, currently MiniMax/provider routing.
- `aiv3-codex-host-agent` exists but is configured plan-only / shim mode with real execution disabled.
- Codex diagnostics may remain as dormant reference/observability, but Codex is not the ordinary answer path.

Do not:

- Log the 8 server Codex CLI into the operator GPT account.
- Store GPT/ChatGPT account credentials for V3 service use.
- Enable real Codex transports or `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC`.
- Route ordinary AssistantRun, external bot, or third-party chat answers through Codex.
- Build a Codex-backed model proxy unless the operator explicitly reopens the track.

Related account decision:

- ChatGPT Plus/Pro subscriptions do not provide API credits for V3 backend use.
- Home model lease may manage provider API keys / model proxy leases, but should not lease a `codex-web` ChatGPT/Codex login session as V3 shared model service.
- If V3 needs GPT models, use OpenAI Platform API keys and API billing through `llm-gateway` / provider routing.

## Current Product Direction

The UI shell is stable. Do not redesign the left dataset rail, top toolbar, home-only composer, right output shelf, directory pages, static-page entry card, or single `效果图——生成页面` CTA unless fixing a concrete blocking bug.

Current active mainline:

- Static-page generation quality: planning, module evidence binding, chart/data fidelity, image-preview prompt quality, final render fidelity, export diagnostics.
- Assistant/RAG quality: V3 awareness, visible dataset scope, document detail supply, title/heading-aware retrieval, hidden conversation memory, evidence boundaries.
- Ordinary AssistantRun provider quality: provider routing, fallback discipline, model-facing context, no fake evidence, no hidden observability in normal chat.
- Dataset/document reading UX: document detail page, markdown source display, chunks/evidence visibility, previous/next document switching.

Current hold/freeze states:

- PPT/video extraction is frozen unless explicitly resumed. Keep existing implementation as baseline; do not expand richer PPTX/OCR work for now.
- Third-party integration is mostly ready for external coordination. Do not overbuild without a real customer sandbox endpoint; maintain docs/smoke/readiness only when needed.
- Codex substrate is frozen as described above.

## Recent Baseline

Latest committed local work before this handoff:

- `a293500 Add dataset document detail page`
- `ae03d01 Render document source as HTML reading view`
- `eefd017 Show document source as markdown`
- `581ab25 Add ingest runtime MarkItDown gate`
- `fc06547 Make document markdown source scrollable`

The current document-reading path:

- Dataset page can open a document detail view from a document name.
- The detail page can show document metadata, chunks, retrieval evidence, model-facing signals, and source text.
- Source display is currently Markdown-oriented and scrollable.
- MarkItDown is installed/pinned as fallback parser capability, but V3's native parsing chain remains the primary path.
- Deployment/runtime gate should check `PYTHON_BIN -m markitdown --version`; current validation doc says installed fallback is `markitdown 0.1.5`.

## Non-Negotiable Rules

- No fake data. Missing or partial evidence must stay visible to model, UI, renderer, and export package.
- No host-composed assistant answer. V3 supplies context/evidence/actions; the model writes final text.
- V3 remains authority for dataset visibility, memory scope, workflow state, queue submission, action validation, model routing, and artifact ownership.
- Dataset selection is supply preference, not a separate chat mode.
- Every model-facing turn must include additive V3 awareness: what V3 is, what V3 can do, which permission-scoped datasets/tools are visible, and what is not supplied.
- If V3 has no visible evidence or permission for a topic, model output should say `当前不可见/未供料` before continuing with clearly labeled general knowledge if useful.
- External/web search is planned as V3-controlled read-only evidence. Do not claim live search unless V3 supplied audited search evidence with source/time metadata.
- Normal chat should not show execution observability by default.
- Do not delete files directly. Use `C:\Users\soulzyn\.codex\bin\Safe-RemoveToBackup.ps1` for cleanup.
- Never commit `.storage`, provider tokens, env files, queue credentials, local smoke artifacts, or GPT/Codex account credentials.

## Suggested Next Work

Recommended first closure for the next thread:

1. Reconfirm the current worktree and commit or preserve the documentation-only freeze/handoff changes.
2. Start from the document/RAG quality lane, because the latest local commits are already in that area.
3. Run or add a narrow smoke around dataset document detail/source display:
   - open dataset page;
   - click document name;
   - verify source Markdown can scroll to full content;
   - switch previous/next documents;
   - confirm chunks/evidence/model-facing signals remain visible and do not fake missing source.
4. Continue improving retrieval/context quality using document paragraph headings as stronger RAG signals, as requested by the operator.
5. After document/RAG closure, return to static-page quality: ensure static-page planning uses better evidence/title signals before image preview or final render.

Keep third-party, PPT/video, and Codex tracks in their current hold state unless the operator explicitly reopens one.

## 2026-05-18 Addendum

After this handoff was created, the local thread continued into the document/RAG lane:

- Document detail chunk views now standardize inferred Markdown/paragraph heading clues into `metadata.section_title_hints`.
- Document detail `model_facing.signals` now includes heading-clue counts and compact heading samples as a RAG signal.
- Static-page draft planning data now carries supplied heading clues as a `retrieval.section_title_hints` field candidate; `docs-page` structure modules bind to that candidate in frontend and backend snapshots so document-page templates can preserve source structure when evidence already provides headings.
- Docs-page missing-evidence state now treats supplied `section_title_hints` as satisfying the heading/detail gate, so the ReAct path can proceed without repeatedly asking for document detail when source headings are already available.
- Static-page runtime provider input now includes compact `structure_signals` with supplied section-title hints, matching field candidates, and bound docs-page modules; provider instructions tell the model to use those as source structure clues and never invent headings.
- AssistantRun/ReAct current static-page artifact summaries now include compact `structureSignals` for the same heading clues, while still omitting module body text and sample rows.
- Frontend `dataSnapshot.structureSignals` and backend `dataSnapshot.structure_signals` now carry the same source-structure contract on the draft itself, so the signal is not limited to provider/ReAct context.
- The desktop/mobile static-page template reference panel now shows a compact "源结构" summary for supplied heading clues and bound docs-page modules without exposing document body text or chart sample rows.
- The static-page planning handoff HTML artifact now includes a read-only "源结构" section sourced from structured JSON, keeping Markdown/JSON as the source of truth and avoiding raw provider HTML.
- A repeatable "新世界 IOA" static-page planning smoke fixture now lives at `docs/smokes/new-world-ioa-static-page-handoff.json`; `npm run build:static-page-planning-smoke-html` renders ignored review HTML/JSON under `target/html-artifacts`, and `npm run test:static-page-planning-smoke-html` verifies the safe HTML sections.
- Existing retrieval ranking tests still cover section-title hints for indexed evidence and fallback chunk supply.
- Focused validation passed:
  - `cargo test -p platform-api load_document_detail_returns_document_chunks_and_retrieval_evidences --lib`
  - `cargo test -p platform-api to_document_chunk_view_exposes_typed_state --lib`
  - `cargo test -p platform-api select_retrieval_evidence_ids_for_prompt_prefers_section_title_hint --lib`
  - `cargo test -p platform-api rank_document_chunks_for_prompt_prefers_section_title_hint_for_fallback_supply --lib`
  - `cargo test -p platform-api docs_page_data_snapshot_binds_structure_modules_to_section_title_hints --lib`
  - `cargo test -p platform-api docs_page_missing_evidence_is_ready_when_section_title_hints_are_supplied --lib`
  - `cargo test -p platform-api docs_page_draft_creation_uses_supplied_section_title_hints --lib`
  - `cargo test -p platform-api assistant_run_react_provider_input_warns_against_rendering_stale_static_pages --lib`
  - `cargo test -p platform-api assistant_run_provider_input_summarizes_current_static_page_without_body --lib`
  - `cargo test -p platform-api assistant_run_react_continue_provider_input_warns_against_stale_static_page_render --lib`
  - `cargo test -p static-page-runtime --lib`
  - `node --test app/lib/static-page-draft.test.mjs`
  - `node --test app/lib/document-detail-view.test.mjs`
  - `node --test app/lib/html-artifact-manifest.test.mjs`
  - `npm run build:static-page-planning-smoke-html`
  - `npm run test:static-page-planning-smoke-html`
  - `npm run build`
  - `git diff --check`

## Key Files

Plans and handoffs:

- `docs/plans/2026-05-07-v3-master-development-plan.md`
- `docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md`
- `docs/plans/2026-05-13-v3-external-bot-third-party-knowledge-plan.md`
- `docs/integrations/third-party-integration-api.md`
- `docs/integrations/third-party-integration-api.zh-CN.md`
- `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- `docs/validation/ingest-runtime-dependency-gate.md`

Document/RAG:

- `apps/web/app/HomePageClient.js`
- `apps/web/app/components/WorkspaceDirectoryPanel.js`
- `apps/web/app/globals.css`
- `crates/platform-api/src/lib.rs`
- `crates/platform-api/src/react_agent_tools.rs`
- `crates/ingest-worker/src/lib.rs`
- `crates/retrieval-worker`
- `crates/contracts/src/lib.rs`

Static page:

- `apps/web/app/lib/static-page-draft.js`
- `apps/web/app/components/static-page/`
- `crates/static-page-runtime/src/lib.rs`
- `crates/static-page-renderer/src/lib.rs`
- `crates/static-page-worker`

Provider / AssistantRun:

- `crates/assistant-runtime/src/lib.rs`
- `crates/llm-gateway/src/lib.rs`
- `crates/platform-api/src/lib.rs`
- `apps/web/app/lib/assistant-startup-briefing.js`
- `apps/web/app/lib/scope-planner.js`

Frozen Codex reference:

- `crates/codex-host-agent/`
- `docs/architecture/codex-host-bridge-contract.md`
- `docs/operations/codex-host-model-profiles.md`
- `docs/operations/codex-jump-host-minimax-smoke.md`

## Verification Commands

Use focused commands first:

```powershell
git status --short
git diff --check
node --test apps/web/app/lib/static-page-draft.test.mjs
npm run test:static-page-planning-smoke-html
Push-Location apps/web; npm run build; Pop-Location
```

For document/RAG or ingest changes:

```powershell
cargo test -p platform-api document_detail
cargo test -p platform-api retrieval
cargo test -p ingest-worker
bash scripts/run-ingest-runtime-gate.sh
```

For AssistantRun/provider changes:

```powershell
cargo test -p assistant-runtime
cargo test -p llm-gateway
cargo test -p platform-api assistant_run
```

For static-page changes:

```powershell
cargo test -p static-page-runtime
cargo test -p static-page-renderer
cargo test -p static-page-worker
cargo test -p platform-api static_page
```

Do not run or validate real Codex execution while the Codex substrate is frozen.

## Fresh Thread Prompt

```text
Continue AI Data Platform V3 from C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3.

Read first:
- docs/handoffs/2026-05-18-v3-thread-handoff.md
- docs/plans/2026-05-07-v3-master-development-plan.md

Current local HEAD at handoff creation: fc06547 "Make document markdown source scrollable".

Important: the Codex substrate is frozen as of 2026-05-18. Do not log the 8 server Codex CLI into a GPT account, do not enable real Codex transports, and do not route ordinary AssistantRun/external-bot/third-party chat through Codex. V3's active answer path remains provider routing, currently MiniMax/provider.

Keep the UI shell stable. Do not redesign the left dataset rail, top toolbar, home-only composer, right output shelf, directory pages, static-page entry card, or the single "效果图——生成页面" CTA unless fixing a blocking bug.

PPT/video and third-party integration are currently on hold unless explicitly reopened. Third-party is mostly waiting for real customer sandbox coordination.

Start with document/RAG quality: verify the dataset document detail/source page, source Markdown scrolling, previous/next document switching, chunks/evidence/model-facing visibility, and then improve heading/paragraph-title signals for retrieval and static-page planning. Preserve the no-fake-data and current V3-awareness rules.

Before committing, run focused tests plus git diff --check. Commit only related verified changes.
```
