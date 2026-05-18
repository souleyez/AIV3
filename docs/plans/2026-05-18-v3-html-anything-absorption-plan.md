# V3 HTML Anything Absorption Plan

**Date:** 2026-05-18

**Status:** P0 and first P1 slice implemented through ReAct static-page draft creation, template selection, evidence reporting, strict provider planning contract, missing-evidence follow-up routing, docs-page heading-signal binding, draft-level structure signals, and safe HTML handoff rendering.

## Goal

Absorb `html-anything` as a V3-owned rapid-output capability without importing its execution model. `html-anything` becomes a template source and design-reference library. V3 remains the authority for model routing, permissions, dataset visibility, retrieval supply, artifact ownership, renderer behavior, and export packaging.

## Decision

Use `html-anything` for:

- Template taxonomy and metadata.
- Design constraints from `SKILL.md` files.
- Example-driven layout references.
- Export and screenshot workflow ideas.

Do not use `html-anything` for:

- Local coding-agent CLI execution.
- Reusing ChatGPT/Codex/Claude subscription sessions as V3 service credentials.
- Arbitrary model-written HTML as trusted final output.
- Remote scripts, remote CSS, provider secrets, queue credentials, or private paths inside V3 artifacts.
- PPT/video/frame surfaces while those V3 tracks are paused.

## Product Shape

The V3 capability should be called a template-assisted rapid output path:

1. The user asks for a report, dashboard, one-pager, prototype, or card.
2. V3 resolves visible datasets, document evidence, conversation memory, and current artifact context.
3. V3 selects an enabled template reference from explicit choice, existing draft/source metadata, or safe intent inference.
4. V3 routes the planning turn through `llm-gateway` using normal provider profiles.
5. The model receives a template reference and V3-supplied evidence boundaries, then returns structured JSON.
6. V3 compiles that JSON into `StaticPageDraft.modules`, `dataSnapshot`, `visualSpec`, and `renderSpec`.
7. The draft can be rendered quickly as a V3 static page, then optionally upgraded through the existing effect-image and final-render path.
8. Output ownership stays in V3 static-page drafts, render outputs, HTML artifacts, and the right shelf.

## P0 Scope

Start with metadata and draft seeding:

- Add a V3 template-reference registry for a small allowed subset of `html-anything`.
- Enable static-page draft creation to carry a safe `designReferences` array.
- Seed static-page modules from three non-frozen references:
  - `data-report`
  - `dashboard`
  - `docs-page`
- Mark deck/video references as paused examples, not enabled output targets.
- Include design references in image/final-render payloads as guardrails, not as raw HTML.
- Add `template_reference_id` to the AssistantRun static-page draft creation contract.
- Keep template ids behind a backend allowlist and reject paused PPT/video references.
- When the backend receives a template id without a full draft payload, seed the V3 draft with structured modules and mobile order.
- Infer `data-report`, `dashboard`, or `docs-page` from user intent on the frontend draft path.
- Infer the same three references on the backend default-payload path, while preserving full caller-supplied draft payloads unless they explicitly carry a template id.
- ReAct `create_static_page_draft` uses the same backend creation path, so template selection, draft payload seeding, events, and evidence reporting stay consistent.
- AssistantRun events now include `template_reference`, `evidence_summary`, and `missing_evidence` for static-page draft creation.

This does not add a new UI selector yet. The current frontend draft sync passes a selected or inferred template id when one exists on the local draft.

## P1 Scope

- Show the selected template reference in the right shelf or static-page planning handoff artifact.
- Let ReAct follow-up actions use `missing_evidence` to choose `retrieve_evidence`, `read_document_detail`, or `update_static_page_module`.

## Completed P1 Slice

- Promoted template selection into AssistantRun ReAct/static-page planning observations for `create_static_page_draft`.
- Stored selected template id, template reference metadata, evidence summary, and missing-evidence state in AssistantRun draft-created events.
- Built a strict static-page provider prompt/schema for template-assisted planning.
- Passed compact `template_reference` and `missing_evidence` context into the static-page intent runtime.
- Added an `output_contract` for provider responses and runtime rejection for unsafe raw HTML/script-like text in summaries and module text fields.
- Persisted `templateReference`, `templateEvidenceSummary`, and `missingEvidence` on static-page draft payloads.
- Exposed template reference and missing-evidence state in the desktop/mobile static-page planning panels.
- Added the same template/evidence context to the static-page planning handoff HTML artifact.
- Added compact `templateReference` and `missingEvidence` summaries to current static-page artifact ReAct context without leaking module body text or sample rows.
- Updated ReAct start/continue prompts to map `missingEvidence.status=needs_evidence` into `retrieve_evidence`, `read_document_detail`, or `update_static_page_module`, and to avoid preview/final render while blocking evidence is unresolved unless the user accepts a partial draft.
- Preserved supplied document heading clues as `retrieval.section_title_hints` field candidates and bind docs-page structure modules to them in both frontend and backend data snapshots when evidence already contains those clues.
- Treat docs-page heading evidence as satisfying the heading/detail missing-evidence gate, so ReAct does not keep asking for document detail after usable source headings are already supplied.
- Added compact `structure_signals` to the static-page provider input so docs-page planning turns can use supplied section-title hints without copying raw document content or inventing headings.
- Added compact `structureSignals` to current static-page artifact summaries in AssistantRun/ReAct planning so follow-up actions can preserve supplied document structure without leaking module bodies or sample rows.
- Added frontend `dataSnapshot.structureSignals` and backend `dataSnapshot.structure_signals` so source structure clues travel with the draft itself, not only provider/ReAct context.
- Show source structure hints and the docs-page modules bound to them in the desktop/mobile template reference panel, without exposing document body text or chart sample rows.
- Include the same structure signals in the existing static-page planning handoff HTML artifact, keeping Markdown/JSON as source of truth and rendering only a read-only "源结构" summary.
- Added a repeatable "新世界 IOA" static-page planning smoke fixture and renderer that reads a committed structured manifest, writes review HTML only under `target/html-artifacts`, and verifies template reference, missing-evidence, source-structure, visual-bridge, and module-planning sections.

## Guardrails

- Template references may influence style and module recipes only.
- All model-facing facts still come from V3 visible context or clearly labeled general knowledge.
- Missing evidence must remain visible; no template may hide partial or missing data.
- Final HTML comes from V3 renderers or trusted HTML artifact templates, not raw provider HTML.
- `deck-*`, `frame-*`, `video-*`, and Remotion/Hyperframes references remain paused while PPT/video work is frozen.

## Verification

P0 should pass:

```powershell
node --test apps/web/app/lib/html-template-references.test.mjs apps/web/app/lib/static-page-draft.test.mjs
node --test tools/render-static-page-planning-smoke-html.test.mjs
cargo test -p platform-api static_page_template_reference --lib
cargo test -p platform-api assistant_run_react_provider_input --lib
cargo test -p platform-api assistant_run_react_continue_provider_input_warns_against_stale_static_page_render --lib
cargo test -p platform-api static_page_draft_can_be_created_under_assistant_run --lib
cargo test -p platform-api assistant_run_react_can_create_template_assisted_static_page_draft --lib
cargo test -p platform-api assistant_run_react_static_page_module_update_applies_current_backend_draft --lib
cargo test -p platform-api docs_page_data_snapshot_binds_structure_modules_to_section_title_hints --lib
cargo test -p platform-api docs_page_missing_evidence_is_ready_when_section_title_hints_are_supplied --lib
cargo test -p platform-api docs_page_draft_creation_uses_supplied_section_title_hints --lib
cargo test -p platform-api assistant_run_react_provider_input_warns_against_rendering_stale_static_pages --lib
cargo test -p static-page-runtime --lib
npm run build
git diff --check
```
