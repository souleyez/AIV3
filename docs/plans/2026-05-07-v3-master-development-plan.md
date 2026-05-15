# AI Data Platform V3 Master Development Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Provide the single active development plan for AI Data Platform V3, with static-page generation as the product mainline and the model-gateway/Codex conversation executor as the core assistant execution architecture behind V3 control.

**Architecture:** V3 remains the control plane and source of truth: PostgreSQL owns identity, dataset visibility, AssistantRun state, memory scope, workflow state, and artifacts. The model gateway owns provider profiles and routing across GPT, MiniMax, and other OpenAI-compatible or adapted model APIs. The Codex conversation executor receives every assistant conversation as a bounded task, uses V3-supplied context/evidence/tool manifests, decides the next reasoning/tool steps, and returns model-authored messages plus validated action requests. Static-page generation, reports, parsing, retrieval, and media understanding stay as V3 product capabilities; Codex is the assistant execution kernel, not a replacement for V3's API, data plane, authorization, or durable workflow plane.

**Tech Stack:** Next.js 16 / React 19 in `apps/web`; `react-grid-layout` for desktop module layout; `@dnd-kit` for mobile vertical ordering; Apache ECharts for advanced chart/runtime parity; deterministic HTML/SVG rendering for export-safe fallback; sandboxed HTML artifact renderer for task reports/planning handoffs/lightweight editors; Rust crates `platform-api`, `assistant-runtime`, `llm-gateway`, future `model-proxy` facade when extraction is justified, `static-page-runtime`, `static-page-worker`, `static-page-renderer`, `ingest-worker`, `media-worker`, `assistant-run-worker`, `retrieval-worker`, `memory-worker`, `document-vlm-runtime`, `codex-host-agent`; PostgreSQL 17.9 target; Cloudflare/Codex image queue; MiniMax VLM/media capability probes; OpenAI `openai/codex` OSS as the execution-kernel reference and host binary/SDK surface; Codex CLI/SDK/app-server/MCP server on jump host or later Mac host; OpenClaw only as optional legacy sidecar.

---

## Status

This is the current active plan as of 2026-05-10.

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
- Route assistant conversations through a Codex execution kernel once the gateway bridge is ready, with V3 supplying context, evidence, memory, tool contracts, and safety gates.
- Generate static-page/report artifacts inside the main assistant workspace.
- Let users adjust modules by natural language and lightweight direct manipulation.
- Open safe HTML artifacts for planning handoff, execution reports, code review summaries, and lightweight JSON-patch editors.
- Expose the same AssistantRun, retrieval, permission, artifact, and action-validation capabilities through external bot and third-party chat surfaces, including Feishu/Lark, WeCom, and customer-hosted pages.
- Preserve finished outputs and drafts in the right shelf.
- Keep permissions, memory, and artifacts scoped to user/account/local-key visibility.

## Non-Negotiable Rules

- No fake data. If evidence is missing, the UI and generated artifact must say data is missing or partial.
- No local answer composition. V3 supplies context and actions; the model writes the answer.
- Codex may decide reasoning and action steps, but V3 remains the authority for permissions, visible datasets, memory scope, workflow state, queue submission, and artifact ownership.
- The model gateway must support multiple provider APIs through explicit profiles, redacted credentials, provider capability manifests, and per-lane fallback policy.
- Dataset selection is supply preference, not a separate chat mode.
- Conversation history is a hidden dataset and enters context only through scope policy.
- Foreground upload work only saves, preclassifies, registers, and enqueues. Heavy parsing is background work.
- Static-page planning lives in the main workspace, not a separate popup.
- The right panel remains drafts and finished outputs.
- The overall UI shell is frozen unless the user explicitly reopens UI redesign. Keep the current home/dataset/data-source/member/audit layout, left dataset rail, top toolbar, right output shelf, and home-only composer. Future work should improve backend parsing quality, planning quality, generation quality, retrieval quality, render quality, and execution reliability rather than reworking the interface.
- Module-level editability is a core feature, not a nice-to-have.
- HTML artifacts must be sandboxed V3-owned render templates. Do not display raw provider/model HTML with scripts, remote assets, secrets, queue credentials, or direct database actions.
- Account/auth work must never store raw local keys, OTP codes, provider tokens, or session cookie values.
- Codex real execution must not run on this developer workstation. Real execution validation is only for the jump host or later Mac host.
- OpenClaw is optional and should not distract from the product mainline unless a concrete bug appears.
- External bot and third-party integrations are surfaces, not new authorities. V3 must still own effective permissions, AssistantRun state, retrieval supply, artifact access, action validation, and audit.

## Current Baseline

Completed and preserved:

- Original-assistant-style web shell with left dataset rail, main workspace, right output shelf, mobile support, upload entry, and one visible static-page/report action.
- Desktop information architecture is now fixed: the left rail only selects datasets, the floating top toolbar owns system status/login status/model proxy/page directory, the bottom composer owns send/upload/page actions on the home page, and the main workspace switches between assistant home, dataset management, data sources, members, and audit. Do not keep tuning the overall shell unless a concrete usability bug blocks the product flow.
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
- Codex plan-only action suggestions, the real static-page preview queue, and final static-page render now consume static-page `bindingQuality` summaries: if chart/module data still needs attention, the executor or backend gate suggests module repair or retrieval instead of submitting an effect-image preview or final render; confirmed data quality still allows the normal `效果图——生成页面` path. The web request layer preserves structured gate details so module-level repair hints are not lost before reaching the UI.
- Home UI shell is now close to fixed: homepage keeps the conversation composer, directory pages remove the composer, the left rail remains dataset-only, the top toolbar owns page navigation/login/model status, the right shelf owns drafts/results, and members shows an explicit login-required overlay instead of looking broken.
- Java 8 parity audit is complete: Java/Vue used `gridstack` plus `echarts`; V3 keeps `react-grid-layout` and adds ECharts as advanced chart runtime.
- Model gateway seed exists in `llm-gateway` with model lanes, provider error redaction, and MiniMax reasoning block cleanup.
- Codex Host safe bridge exists in dry-run/plan-only form. Real execution is still blocked by default.
- The next architecture uplift is to connect model-gateway profiles to the Codex conversation executor so normal assistant conversations can be judged/executed by Codex while V3 supplies data and validates actions.
- `openai/codex` OSS has been checked as the target execution-kernel reference. It is Apache-2.0, Rust-majority, installable by npm/Homebrew/releases, and exposes multiple integration surfaces: `codex exec` non-interactive runs, structured output schemas, SDK thread control, app-server JSON-RPC, and MCP server mode.
- `CoDeepSeedeX` has been checked as a reference pattern for private Responses-compatible provider shims. It is useful for Codex-to-non-OpenAI provider adaptation, profile wrappers, health/usage/debug endpoints, context-budget diagnostics, tool-call protocol repair, and liveness guards. It is not a V3 dependency or replacement architecture.
- Video PPT/transcript extraction is partially present: V3 can parse uploaded/local audio-video evidence, preserve media timestamps, expose model-visible `resolve_video_url` / `extract_video_ppt_transcript` action contracts, recognize direct video URLs as remote media sources, resolve simple public HTML pages that expose `<video>`, `<source>`, or OpenGraph/Twitter video fields, auto-register the resolved direct video URL into the selected visible dataset and enqueue ingest, let ReAct supply parsed transcript/scene/keyframe OCR evidence from selected uploaded video documents without host-composed answers, queue `video_extraction_workflow` from extraction requests when selected video evidence is still missing, let `media-worker` consume `media` queue stages and advance source resolution/asset registration/extraction-summary stages, let `media-worker` emit an FFmpeg/raw_frames extraction plan compatible with `wechat-video-ppt-extract`, optionally execute local-file FFmpeg extraction behind `MEDIA_FRAME_EXTRACTION_ENABLED`, let `ingest-worker` claim both normal ingest and `parse_video_media` tasks by default, let `ingest-worker` download explicitly enabled direct remote media URLs into a guarded local cache before parsing, emits a safe read-only `video_extraction_summary` HTML artifact from parsed evidence into AssistantRun events/right-shelf discovery, promotes generated text/review files, raw frame manifests, raw contact-sheet HTML previews, keep-list-derived selected slide manifests, final deliverable manifests, and basic screenshot-based PPTX outputs into standard artifact refs, exposes explicit deliverable status in worker output/HTML summaries, has shared request/source/asset/artifact/state contracts, has internal tool catalog entries for `media.resolve_video_url`, `media.register_video_asset`, and `media.extract_ppt_transcript`, and has a dedicated `video_extraction_workflow` stub with `resolving_source -> registered -> parsing -> extracting_ppt -> completed` plus `failed`/`unsupported_source` branches. Richer PPTX/Markdown extraction artifacts and durable final deliverable publication are still pending. Login-gated video sites, QR login, cookies, redirects, private hosts, and recording bypass flows are out of scope for the next implementation slice.

## Architecture Modules

Use these seven modules for all future design and implementation decisions.

### 1. Experience UI

Owns assistant shell, main workspace, dataset display, mobile interactions, module editing UI, static-page/report views, and right shelf.

Also owns safe artifact viewers for static-page handoff previews, Codex execution reports, code review/project inventory summaries, and lightweight HTML editors that emit JSON patch requests back to V3.

Does not own model routing, dataset visibility decisions, direct queue access, database access, Codex Host flags, or execution of arbitrary HTML/JavaScript from providers.

The Experience UI should now be treated as product-stable. Only make narrow fixes for blocking usability, mobile breakage, accessibility, or state visibility. New capability work should surface through the existing shell rather than adding new global panels, new chat inputs, or new static-page flow screens.

### 2. V3 Control Plane

Owns `platform-api`, user/session/local-key semantics, dataset/document visibility, AssistantRun state, draft/output ownership, workflow submission, runtime inspect, and artifact ownership.

Does not own provider-specific HTTP details or local host execution.

### 3. Model Gateway / Integration Plane

Owns `llm-gateway`, future `model-proxy` facade if needed, provider profile configuration, model lanes, provider capability manifests, provider fallback policy, credential redaction, `tool-registry`, `mcp-gateway`, and prompt policy.

The gateway must be able to configure multiple model APIs, including GPT-family providers, MiniMax-compatible providers, and future OpenAI-compatible or adapter-backed providers. Do not create a separate network service until at least two independent processes need the same provider facade; before that, strengthen `llm-gateway` as the shared library/facade.

### 4. Codex Conversation Executor

Owns the bounded conversation-execution loop: receive an AssistantRun context package from V3, call Codex with the configured model profile, let Codex reason over supplied context/tool contracts, emit concise progress steps, request V3-validated actions, and return model-authored assistant messages or artifact/action intents.

The first integration should use supported `openai/codex` surfaces rather than forking the Codex core: TypeScript SDK/app-server for long-lived conversation threads when stable, `codex exec --output-schema` for non-interactive worker tasks, and `codex mcp-server` only as a later tool boundary if it is operationally cleaner.

Does not own dataset visibility, memory source of truth, provider credentials, direct database reads, direct queue writes, static-page rendering, file parsing, or artifact ownership. It asks V3 for supply/action execution; V3 decides what is visible and what action is allowed.

### 5. Workflow / Worker Plane

Owns explicit workflow definitions, queue claiming, ingest/retrieval/memory/report/static-page/media workers, retry/cancel/dead-letter/replay semantics, background task status, and Codex execution tasks that need durable worker scheduling.

Do not move routine parsing, retrieval, rendering, or memory refresh into the Codex conversation executor.

### 6. External Execution Host

Owns `codex-host-agent`, host profile validation, isolated task workspace, guarded `codex exec`, redacted logs, returned artifacts, and workflow completion.

It is not a browser API, model gateway, memory authority, permission authority, or V3 scheduler. For normal chat, it should be treated as the host process for the Codex conversation executor; for heavier automation, it is a workflow worker.

### 7. Data / Artifact Plane

Owns PostgreSQL, object storage assets, vector indexes, analytical files, published static-page/report artifacts, migrations, and backup/restore discipline.

Also owns durable HTML artifact records, manifests, provenance, versioning, and export packages. HTML artifacts are products of trusted V3 templates plus sanitized data, not arbitrary browser documents.

Fresh production-like environments target PostgreSQL 17.9.

## Roadmap Priority

### Priority 1: Static-Page Product Mainline

This is the next active development direction.

Current evidence-manifest follow-up: the third-party handoff package now writes a final `<package>.evidence-manifest.json` after aggregate evidence, and `validate:evidence` verifies every final delivery artifact plus `.all.json`/`.all.md` without introducing a circular delivery-manifest hash dependency.

Current readiness follow-up: deployment readiness reports that receive `--releasePackage` now also validate the final evidence manifest and expose `handoff_evidence_ready` plus a redacted `handoff_evidence_summary`.

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

The visible UI pattern is already selected. Future static-page work should focus on better planning, better module content/data binding, stronger retrieval supply, stronger image-preview prompts, better deterministic final render fidelity, and cleaner export diagnostics. Avoid broad visual redesign of the assistant shell.

### Priority 2: Model Gateway + Codex Conversation Executor

This is now a core architecture track, not a distant sidecar.

The target flow is:

1. Browser sends a conversation turn to V3.
2. V3 creates or continues an `AssistantRun`.
3. V3 builds a supply package: system/product briefing, visible datasets, selected/inferred scope, hidden-memory candidates, evidence, current artifact state, tool catalog, and safety policy.
4. V3 sends the task to the Codex conversation executor.
5. Codex uses the configured model profile through the model gateway, decides whether to answer, retrieve more, update a draft, request an image preview, render a page, recall memory, or continue execution.
6. V3 validates every requested action against user/session/dataset/artifact scope.
7. V3 persists events, supplied evidence, actions, artifacts, and the final model-authored response.

For static-page turns, Codex action planning must respect V3's data-quality gates. A request for an effect image is not enough by itself: if the current artifact reports missing bindings, matched-field-only candidates, inferred chart signals, or chart data without sample rows, the executor should first request retrieval or module repair through V3-validated actions. This keeps the GPT Image bridge and final renderer from turning weak evidence into polished-looking fake dashboards.

The product must support:

- Gateway-configured model profiles for multiple provider APIs.
- Per-profile capability manifests: chat, reasoning, vision, audio/video, JSON mode, tool calling, image-prompt support, rate limits, and cost hints.
- Codex as the normal assistant reasoning/execution kernel once the bridge is stable.
- A non-fork integration with upstream `openai/codex`: use official binary/SDK/server surfaces first, keep a local checkout only for debugging, Python SDK experiments, or patch evaluation.
- A V3-owned private Responses-compatible shim pattern for providers Codex cannot call natively, using `CoDeepSeedeX` as a design reference but not as a runtime dependency.
- Provider-shim observability: local-only health/status, capability, balance/usage, trace, and context-budget diagnostics. Persist product-grade usage/audit into V3 PostgreSQL/runtime inspect rather than relying on shim-local SQLite as source of truth.
- Context budget governance: diagnose and cap runaway conversation history, tool outputs, retrieval payloads, and media/image payload summaries while preserving recent/high-risk evidence. This is required because the product intentionally favors answer quality over token thrift.
- Tool-call protocol hardening: repair malformed tool-call/tool-output pairing before provider calls where safe, detect tool-call liveness stalls, and surface retry/continue steps as AssistantRun events.
- V3-supplied context instead of Codex directly reading databases or local files.
- ReAct progress surfaced as brief UI steps, not a verbose terminal log.
- Provider fallback without silently changing safety/capability assumptions.
- Jump-host/Mac-host validation for real Codex execution; no real local workstation execution.

This track must not disrupt the current static-page user flow. Codex can replace the reasoning/action planner behind a feature gate, but V3 keeps the existing static-page state machine, draft model, preview queue, final renderer, artifact ownership, and current UI progression. `direct` execution remains the fallback until Codex-backed action planning proves equivalent or better in shadow runs.

### Priority 3: Assistant/RAG/Ingest Quality

Continue improving supply quality, not UI form complexity:

- Better scope planner candidates.
- More aggressive RAG/detail supply when selected or inferred datasets match.
- Hidden conversation memory retrieval only when useful.
- Better document structured profiles from MiniMax VLM metadata.
- Media detail API for transcript windows, scenes, keyframes, OCR snippets, and provider evidence.
- Direct-upload and publicly resolvable video acquisition: the assistant should know it can parse uploaded video files, direct video URLs, or public pages where V3 can safely resolve a video asset URL, then feed that media into the background parsing and PPT/transcript extraction pipeline. Login-gated pages, QR login, cookies, and browser recording bypass are explicitly excluded for now.
- No foreground parsing beyond save/preclassify/register/enqueue.

### Priority 4: External Bot And Third-Party Knowledge/Action Integrations

V3 must gain an external integration mainline without becoming a platform-specific chatbot fork.

Detailed implementation plan: `docs/plans/2026-05-13-v3-external-bot-third-party-knowledge-plan.md`.

Third-party-facing API guide: `docs/integrations/third-party-integration-api.md`.

Third-party sendable guide (CN): `docs/integrations/third-party-integration-api.zh-CN.md`.

Current checkpoint: the external bot and third-party integration smoke passed on deployment target `8服务器` at commit `fac2178`, including PostgreSQL-backed ACL/source/adapter/action checks and the public `https://v3.elepcloud.com/external-integrations` panel probe. The follow-up mock/sandbox layer passed on `8服务器` at commit `aaafd6f`, covering customer-hosted generic chat page events and signed outbound third-party dispatch. The standalone packaged gateway smoke passed on `8服务器` at commit `0cfb9c9`, proving that V3's real dispatch client can reach an independently running third-party mock gateway with valid Bearer, HMAC signature, and body hash. The action result callback roundtrip smoke passed on `8服务器` at commit `120933a`, so external systems can now report async outcomes through an HTTP V3 callback route while V3 stores only status, request id, idempotency key, and structural summaries. Commit `c3f702c` surfaces those callback states in `action_summary`, action audit summaries, and the standalone observability panel; deployment-target gateway smoke and web contract tests passed. Commit `15d700a` adds explicit audit filters for action/result-callback states so operators can focus the timeline without reading every message/sync event; deployment-target gateway smoke and web contract tests passed. Commit `a203a54` adds per-action audit drilldown through `action_id` queries and the standalone panel detail view; deployment-target gateway smoke and web contract tests passed. Commit `36b5c28` adds deployment-target readiness reports for signed dispatch, result callback, redaction, and remaining third-party handoff items; deployment-target readiness tests and gateway smoke passed. Commit `c751485` adds action-detail permalink and redacted trace export for operators; deployment-target web contract tests, Next build, gateway smoke, and readiness report passed. Commit `67327ab` adds the third-party handoff manifest sample and validator so customer sandbox readiness can be checked before live joint testing; deployment-target handoff tests, manifest validation, readiness report, and gateway smoke passed. Commit `51c6329` folds handoff manifest validation into the deployment-target readiness report and gateway smoke, adding `handoff_manifest_ready` and `handoff_manifest_summary` to generated reports; deployment-target readiness tests, manifest validation, and gateway smoke passed. Commit `86f6aca` adds a self-contained third-party handoff package builder with public guides, the manifest sample, validation tooling, mock gateway reference, README files, and SHA256 package manifest; deployment-target package tests, package generation, and package-internal validation passed. Commit `b9b89e0` adds package integrity validation for generated handoff packages, including relative-root, path traversal, file size/SHA256, and handoff manifest checks; deployment-target package integrity tests and package-internal `validate:package` passed. Commit `8ac660c` adds sendable `.tar.gz` archive output with a matching `.sha256` sidecar for generated handoff packages; deployment-target archive generation and package-internal validation passed. Commit `37d02c1` adds archive-level validation for sendable handoff packages, including sidecar digest checks, gzip/tar parsing, required archive entries, path traversal safety, package manifest file hashes, and handoff manifest readiness; deployment-target archive tests, archive generation, archive validation, and package-internal validation passed. Commit `e48b563` includes the archive validator and `validate:archive` script inside the third-party handoff package, so package recipients can validate the sibling `.tar.gz` and `.sha256` sidecar from the package directory; deployment-target package tests, archive tests, package generation, and package-internal `validate:handoff`/`validate:package`/`validate:archive` passed. Commit `ab38d8e` adds combined handoff release validation through `tools/validate-external-handoff-release.mjs`, root script `validate:external-handoff-release`, and package-internal `validate:release`; deployment-target release tests, package generation, and root/package release validation passed. Commit `bb7ea22` makes the handoff package builder automatically write a sibling `.release.json` validation receipt with release report SHA256, so delivery evidence is produced with the package; deployment-target package/release tests, package generation, release report existence check, and root release validation passed. Commit `a29428a` adds a human-readable `.release.md` handoff release summary alongside the machine-readable receipt; deployment-target package/release tests, package generation, Markdown existence/content checks, and root release validation passed. Commit `63c4e05` exposes package provenance in release reports, including package type, generated time, and V3 repository head at the top level, in package summaries, and in Markdown receipts; deployment-target package/release/integrity tests, package generation, JSON provenance checks, Markdown provenance checks, and root release validation passed. Commit `d50d282` adds a sibling `.delivery-manifest.json` listing the expected delivery artifacts and SHA256 values for receive-side verification; deployment-target package/release/integrity/archive tests, package generation, root release validation, and delivery artifact role checks passed. Commit `7e03b4e` folds that delivery manifest into combined release validation, checking required artifact roles, expected paths, byte sizes, and SHA256 values after package build; local package/release/archive suites passed. Commit `3ea49bf` gates deployment-target readiness reports on a validated sendable handoff release package, adding redacted `handoff_release_summary` and `handoff_release_ready`; deployment-target readiness tests, release tests, package build, and synthetic readiness report validation passed on `8服务器`. The current local hardening adds receive-side `validate:delivery` plus aggregate `validate:all`, and readiness reports now include `handoff_all_ready` / `handoff_all_summary` whenever a release package is supplied; local all/package/delivery/release/integrity/archive/readiness tests and generated package validations passed. The current evidence follow-up lets root and package-internal `validate:all` persist aggregate JSON and Markdown receipts with `--out` and `--markdown`, so the full customer handoff gate can be attached to delivery review records. Validation records: `docs/validation/external-bot-third-party-deployment-smoke-2026-05-14.md`, `docs/validation/external-third-party-mock-sandbox-smoke-2026-05-14.md`, `docs/validation/external-third-party-gateway-smoke-2026-05-14.md`, and `docs/validation/external-action-result-callback-smoke-2026-05-14.md`.

The product must support:

- Standard Feishu/Lark and WeCom bot/channel adapters for official message, event, card/file reply, callback, signature, replay-protection, and tenant-routing interfaces.
- Pure third-party interfaces for document APIs, user/directory APIs, artifact APIs, action APIs, and chat-channel APIs, so customers can host their own chat pages or portals on separate servers.
- Hybrid deployments where the chat surface, document library, artifact store, and business transaction system live on different servers.
- Third-party document parsing through the existing V3 ingest/retrieval workers, with source document ids, revisions, provenance, and ACL snapshots preserved.
- Effective permission resolution from external user identity, source users/departments/groups/roles, source document ACLs, V3 tenant/account policy, dataset visibility, and channel policy.
- Permission-filtered retrieval before model context supply. Hidden documents must not enter model context and must not be left for the model to ignore.
- V3-validated external transactions with explicit risk levels, confirmation requirements, audit records, and redacted failure summaries.
- An observe-first V3 management UI for connection health, sync status, permission drift, message/run traces, retrieval quality, transaction status, and retry/disable/resync/secret-rotation controls.

Do not add a second V3 chat surface for this track. External chat pages may live outside V3, while V3's own UI stays focused on observability and governance.

### Priority 5: Safe HTML Artifact Layer

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

### Priority 6: Account/Auth Maintenance Only

Account work is currently paused after second-round hardening.

Only fix:

- Access leaks.
- OTP/security bugs.
- Session/cookie breakage.
- Migration/runtime blockers.

Do not start team sharing, robot ownership, admin audit UI, or deep encryption recovery until product mainline is stable.

### Priority 7: OpenClaw Frozen

OpenClaw optional provider/stubs completed their first useful pass.

Do not continue OpenClaw as a main execution-kernel route.

## Overall Architecture Review

The project should stay split into three tracks:

- Product track: assistant context supply, dataset/RAG/media quality, static-page planning/editing, preview image, final render, export package, and right-shelf artifact lifecycle.
- Conversation-kernel track: model gateway profiles, Codex conversation executor, V3 context supply packages, validated action loop, and concise progress reporting.
- Execution extension track: heavier Codex Host workflow tasks, task memory spaces, jump-host/Mac-host validation, and HTML execution reports.

The product track must remain safe without real local host execution, but the normal assistant path should move toward Codex as the reasoning/execution kernel. V3 still answers product questions through model-authored output, retrieves evidence, generates reports/static pages, and manages artifacts through its own API/worker/data planes; Codex decides the next step and asks V3 to perform authorized actions.

The next highest leverage order is:

1. Preserve the current overall UI and stop broad shell redesign.
2. Finish static-page module editability, planning quality, final-render fidelity, export diagnostics, and data-quality handling.
3. Improve parsing, retrieval, hidden conversation memory, media understanding, and AssistantRun context supply so the model sees better evidence and current draft state.
4. Continue direct-upload / publicly resolvable video PPT extraction as a media-quality subtrack: complete the public video URL/page resolver, remote media registration, background media parsing, transcript/PPT extraction artifacts, and no host-composed fallback answers. Uploaded video evidence supply through ReAct already exists.
5. Build the model-gateway profile system and Codex conversation executor bridge behind feature flags and shadow/dry-run comparison, without changing the visible static-page flow.
6. Add the external bot and third-party integration core: generic channel/source contracts, ACL-aware third-party retrieval, Feishu/Lark and WeCom adapters, external artifact/action runtime, and observe-first management UI.
7. Add the safe HTML artifact viewer as a common review/report surface, starting with Codex execution reports, static-page planning handoffs, and video extraction summaries.
8. Validate real Codex/external-page fetching only on the jump host or later Mac host when browser access is needed.
9. Resume account expansion only when product workflows need it.

The main architectural risk is letting four "brains/builders" compete: direct model calls, Codex conversation executor, static-page renderer, and HTML artifact renderer. The boundary is strict: Codex decides conversation/action flow, V3 validates and supplies data, static-page renderer produces customer report pages, HTML artifact renderer displays review/control artifacts, and heavier Codex Host workflow tasks execute external work without owning V3 product state.

## Immediate Execution Track

The next development thread should keep the current UI shell stable and continue with backend/static-page quality: parsing quality, retrieval/context supply, planning quality, image-preview prompt quality, chart/data fidelity, final-render fidelity, and export diagnostics. Start the model-gateway/Codex conversation executor foundation behind feature flags and shadow/dry-run comparison only; do not let it alter the visible static-page workflow until it proves stable.

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

### Task 0A: Promote Model Gateway And Codex Conversation Executor

**Files:**

- Modify: `crates/llm-gateway/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/react_agent_catalog.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/codex-host-agent/src/main.rs`
- Modify: `docs/operations/codex-host-model-profiles.md`
- Modify: `docs/architecture/codex-host-bridge-contract.md`
- Create or update tests near the touched crates

**Status:** In progress. The current baseline has `llm-gateway` model lanes, MiniMax/Codex-shim profile support, ReAct tool contracts, V3-owned Codex context packages, dry-run/plan-only Codex conversation planning, shadow comparison, redacted diagnostics, Host validation summaries, and promotion gates. Direct execution remains authoritative; Codex does not yet route normal assistant conversations as the primary reasoning/execution kernel, and real transport remains blocked until stable shadow comparison plus jump-host/Mac-host smoke validation pass.

**Current implementation note:** `llm-gateway` now has a first-class `ModelProviderProfile` contract with provider/model id, wire API, capability manifest, auth env key name, timeout, rate-limit hints, cost hints, and redaction policy. Public manifests expose only redacted/safe metadata and tests cover GPT-style and MiniMax/Codex-shim profiles without leaking raw keys. It can also convert provider runtime metadata into Provider Shim usage events and summaries for future runtime inspect/PostgreSQL audit persistence. `llm-gateway` also exposes a dedicated `codex_conversation` lane so Codex-backed AssistantRun execution can be routed separately from direct chat/ReAct lanes. `contracts` now has `AssistantRunCodexContextPackageView`, executor transport values, V3-owned safety policy, context budget diagnostics, action contracts, Provider Shim observability snapshots, top-level `supply_quality` and `model_gateway` fields, and a Codex tool-output budget policy so the future Codex executor receives a V3-supplied package and operations can inspect local shim health/profile/usage/budget/liveness state without exposing raw keys or prompts. `assistant-runtime` now has a dry-run/plan-only `execute_codex_conversation_plan` adapter that records safe execution trails, exposes planned action types plus compact supply-quality and model-gateway diagnostics, rejects unsafe context packages, chooses a single safe `suggested_action` with minimal V3-checkable argument skeletons only from V3-provided action contracts in plan-only shadow mode, emits the future `codex exec --output-schema` action-suggestion JSON schema, routes HTML artifact edit prompts through `submit_html_artifact_event`, marks mutation/queue execution as disallowed, emits a host-only invocation blueprint for future `codex exec --output-schema`/SDK/app-server/MCP transports, treats real Codex transports as unsupported until host validation, and always falls back to direct execution without mutating drafts or queues. `platform-api` can now opt in with `ASSISTANT_RUN_EXECUTOR=codex_dry_run|codex_plan_only|codex_exec_schema|codex_sdk_thread|codex_app_server|codex_mcp_server`; it builds a V3-scoped Codex context package for create/continue AssistantRun calls, promotes evidence-state `supply_quality` into the package for explicit Codex guidance, attaches the selected `codex_conversation` model-gateway snapshot/profile through `ASSISTANT_RUN_CODEX_*` route/profile settings, verifies redacted MiniMax/Codex-shim profile loading and runtime-selection fallback in tests, exposes V3 action contracts for retrieval/detail refresh, conversation-memory recall, static-page draft/module/preview/render flow, report flow, and `submit_html_artifact_event`, records concise `assistant_run.codex_executor_diagnostic` events with top-level `suggested_action`, model-gateway summary, host invocation blueprint, and output schema, exposes AssistantRun detail diagnostics for latest Codex shadow status/context-budget pressure/suggested action/model-gateway/host-invocation plus redacted provider usage summaries, computes a shadow gate over recent matched/diverged/invalid/no-suggestion events before allowing jump-host validation, marks suggestions outside V3-provided action contracts as `invalid_suggestion`, and keeps direct execution authoritative. Codex context budget diagnostics now classify prompt/history, startup briefing, selected scope, inferred candidates, retrieval evidence, hidden conversation memory, detail targets, media summaries, current artifact state, and tool/evidence state with estimated character counts and soft-limit pressure. Codex tool-output trimming is narrow and only bounds oversized `tool_outputs` payload fields while preserving recent outputs, error/status fields, evidence references, source locators, and media timestamps; retrieval evidence and media summaries stay quality-first. ReAct/provider protocol hardening now rejects incomplete provider tool-call payloads and repairs duplicate tool-call replay, missing tool outputs, and repeated pending-tool liveness stalls before continuing the action loop. Shadow comparison diagnostics now record direct action types, Codex suggested action type, matched/diverged/no-suggestion/invalid status, selected Codex model-gateway lane/profile, and a hard mutation guard so Codex cannot mutate drafts or queues during shadow evaluation.

**2026-05-10/11 follow-up:** AssistantRun detail diagnostics now also expose a redacted `recent_shadow_events` list for Codex executor diagnostics, including newest-first comparison status, action type, model lane/profile summary, mutation guard signals, and next gate. The latest suggested action is summarized without raw arguments, the latest model-gateway payload is summarized through a fixed safe field whitelist instead of exposing full profile/env/debug data, and the shadow gate reports `matched_streak_count` plus a redacted `last_blocking_event` so operators can see whether failures come from divergence, no suggestion, invalid action type, or unsafe mutation signals. With the jump host available, the shadow gate also emits a `host_validation` readiness summary that says when `windows_jump`/`mac_host` smoke validation can start, while keeping local execution, Codex mutation, and queue submission disabled until host validation passes. AssistantRun diagnostics can also summarize completed Codex Host validation outputs from `codex_exec`/workflow events, provide an aggregate `host_validation_summary`, and combine shadow plus host status into a read-only `promotion_gate` that only marks the path eligible for feature-gate review after both checks pass. Host validation summaries now carry `host_kind` and treat completed `codex_exec` results from anything other than `windows_jump`/`mac_host` as `invalid_host`, not as a promotion signal. Browser-facing real Codex transports are requested/effective separated: unless the manual real-transport feature gate is enabled, `codex_exec_schema`/SDK/app-server/MCP requests are downgraded to `codex_plan_only` and exposed through fixed-field `transport_policy`, `host_invocation`, and `suggested_action` summaries. These summaries deliberately exclude raw prompts, provider secrets, suggested action arguments, command arguments, provider auth env names, transport env keys, host invocation debug fields, stdout/stderr excerpts, and verbose Codex logs.

**2026-05-11 follow-up:** Host validation is now stricter than `codex_exec + completed + allowed host`. A smoke result only counts as validated when it is from `windows_jump` or `mac_host`, actually invoked Codex, used a configured task workspace, kept the command prompt redacted, exited with code `0`, and reported isolated task memory plus a configured task memory space. Completed-looking outputs that fail any of those guardrails are counted as `failed` with `guard_failed_count`, so they cannot make the promotion gate eligible.

**2026-05-11 transport gate follow-up:** Browser-facing real transport requests now require two explicit operator gates before reaching `assistant-runtime`: `ASSISTANT_RUN_CODEX_REAL_TRANSPORT_FEATURE_GATE=enabled` and `ASSISTANT_RUN_CODEX_REAL_TRANSPORT_PROMOTION_REVIEW_APPROVED=approved`. If either gate is missing, V3 records the requested transport in `transport_policy` but downgrades the effective executor to `codex_plan_only`. The promotion-review gate should only be enabled after the read-only `promotion_gate` says shadow comparison and jump-host/Mac-host validation are eligible for feature-gate review.

**2026-05-11 provider-shim diagnostics follow-up:** AssistantRun Codex diagnostics now include a fixed-field `provider_shim_observability` summary when a shadow executor or future shim supplies the contract snapshot. The summary exposes health status, profile/model/wire API, safe usage counts, budget pressure, tool-output trimming counts, and liveness event status, while deliberately hiding auth env names, raw provider errors, request ids, trace ids, raw balance amounts, and diagnostic notes.

**2026-05-11 provider-shim event follow-up:** Codex diagnostic event payloads and UI-readable execution trail entries now synthesize a safe `provider_shim_observability` snapshot for `codex_compatible_shim` profiles even before a real shim process reports health. The synthetic snapshot is intentionally conservative: health is `unknown`, usage is zero, context-budget/tool-output summaries come from the V3 Codex context package, and profile/auth fields stay redacted.

**2026-05-12 jump-host smoke follow-up:** `windows-jump` connectivity was rechecked and Codex CLI 0.123.0 successfully completed a fake local Responses-shim smoke on the jump host with `CODEX_HOST_SMOKE_OK`. The current jump host has no `D:` drive, so the isolated task workspace root for smoke validation is `C:\Users\soulz\codex-host\tasks`. The reusable smoke tool now defaults to request/output summaries rather than raw request bodies or Codex stdout/stderr, fails when Codex never calls the shim or when stdout misses the expected marker, and Windows npm-shim installs should pass `CODEX_HOST_SHIM_CODEX_JS` to the Codex JS entrypoint so Node can spawn Codex reliably.

**2026-05-12 host-output hardening follow-up:** `codex-host-agent` successful `codex_exec` outputs now serialize process `stdout_chars` / `stderr_chars` plus exit code while leaving `stdout_excerpt` / `stderr_excerpt` empty, and failed command errors report log lengths instead of embedding log excerpts. Platform diagnostics continue to derive safe log-length summaries and do not need raw process output for promotion gates.

**OpenAI Codex OSS reference checked on 2026-05-10:**

- Repository: `https://github.com/openai/codex`
- License: Apache-2.0.
- Current public release observed: `0.130.0` on 2026-05-08.
- Primary implementation: Rust-heavy repo with `codex-rs`, `codex-cli`, `sdk`, docs, scripts, and tooling.
- Supported integration surfaces to prefer before forking: `codex exec` for non-interactive work, `--output-schema` for structured final output, `@openai/codex-sdk` for server-side TypeScript thread control, app-server JSON-RPC for richer local control, `codex mcp-server` for MCP tool exposure, and config profiles for model/sandbox/MCP behavior.
- Commercial risk: Apache-2.0 is compatible with commercial use, but OpenAI service usage, model pricing, and account/data handling still follow the selected authentication path and provider terms.

**CoDeepSeedeX provider-shim reference checked on 2026-05-10:**

- Repository: `https://github.com/Awenforever/CoDeepSeedeX/tree/master`
- License: MIT.
- Purpose: local OpenAI Responses-compatible proxy for running Codex with DeepSeek models.
- Useful references: Codex profile wrappers for stable/thinking modes, `/healthz`, status/balance/usage/debug endpoints, usage ledger, context budget diagnostics, persistent/semantic compaction experiments, tool-output trimming, tool-call protocol repair, liveness recovery, and MCP/tool boundary warnings.
- Commercial risk: MIT is compatible with commercial use, but provider API terms, key storage, debug trace retention, and any copied code attribution still need review. Prefer borrowing design patterns and tests over vendoring code.
- Boundary decision: V3 may implement its own private local Responses-compatible shim for MiniMax/DeepSeek-like providers, but the shim only normalizes model API traffic. It must not become the V3 tool executor, permission authority, memory store, queue owner, or product audit source.

**Steps:**

1. Define a first-class model profile contract in `llm-gateway`: provider id, model id, base URL, auth env key name, capability flags, timeout, rate/cost hints, and redaction policy.
2. Add tests proving GPT-style and MiniMax-style profiles normalize request/response metadata without leaking raw keys.
3. Define an `AssistantRunCodexContextPackage` contract containing system/product briefing, selected/inferred datasets, evidence state, hidden-memory candidates, current artifact state, available V3 actions, and safety policy.
4. Add a transport enum for Codex integration: `dry_run`, `plan_only`, `exec_schema`, `sdk_thread`, `app_server`, and future `mcp_server`.
5. Add a Codex conversation executor adapter in `assistant-runtime` that can run in `dry_run` and `plan_only` first, then `exec_schema` using `codex exec --output-schema`, then `sdk_thread` or `app_server` for multi-turn conversations.
6. Route normal `POST /v1/assistant-runs` through the adapter behind a feature/env gate such as `ASSISTANT_RUN_EXECUTOR=direct|codex`.
7. Keep `direct` as the default until jump-host or Mac-host validation proves the Codex path stable.
8. Ensure Codex can only request V3 action contracts such as retrieval/detail refresh, conversation-memory recall, static-page draft creation, module update, image preview submit, final render request, and HTML artifact event submit.
9. Persist Codex reasoning/action summaries as concise AssistantRun events; do not store raw verbose terminal logs in chat.
10. Add UI-readable execution trail entries inside the existing homepage/right-shelf pattern only; do not add a new global execution panel or second chat surface.
11. Validate real Codex execution only on the jump host or later Mac host. Never enable it on this developer workstation.
12. Do not fork or vendor `openai/codex` in V3 unless a concrete upstream gap blocks the supported CLI/SDK/server surfaces. If a patch is needed, keep it as a documented upstream-compatible patch set.
13. Define provider-shim observability contracts inspired by CoDeepSeedeX: health, status, profile/capability snapshot, balance if provider supports it, usage summary, recent usage events, debug trace status, and context-budget report.
14. Store production usage/audit in V3 tables and runtime inspect events. Shim-local ledgers may exist only as host diagnostics and must not become the source of truth.
15. Add context-budget diagnostics before enabling aggressive compaction: classify prompt/system, selected datasets, retrieval evidence, hidden conversation memory, artifact state, tool outputs, and media/image payload summaries.
16. Add conservative tool-output trimming policy for oversized shell/search/file/media outputs in the Codex context package. Trimming must preserve recent outputs, errors, citations/evidence ids, and high-risk failure details.
17. Add tool-call protocol hardening tests for malformed provider responses, missing tool outputs, duplicate function-call replay, and liveness stalls where Codex intended to call a V3 action but returned an incomplete tool request.
18. Run a shadow comparison set for static-page conversations: current direct planner action vs Codex suggested action. Codex suggestions must not mutate drafts or queue work during shadow mode.

**Acceptance:**

- The system can switch a conversation between direct model execution and Codex-backed execution by configuration.
- Codex receives only V3-approved context and tool/action contracts, not unrestricted database/filesystem access.
- Multi-provider model routing is controlled by gateway profiles, not hard-coded per endpoint.
- Codex transport choice is explicit and auditable: `exec_schema` for one-shot tasks, `sdk_thread` or `app_server` for continuing conversations, `mcp_server` only if V3 needs to expose Codex as a tool.
- Non-native model providers can be attached through a private Responses-compatible shim without giving the shim direct V3 database, queue, filesystem, or tool-execution authority.
- Runtime inspect can show provider usage, context-budget pressure, tool-output trimming, and liveness-retry decisions without exposing provider keys or raw sensitive payloads.
- The model can still answer ordinary no-dataset chat when no data is selected.
- Selected/inferred dataset supply still flows through V3 visibility checks before reaching Codex.
- Static-page actions requested by Codex are validated exactly like current ReAct actions.
- The existing static-page UI progression is unchanged while Codex runs in `dry_run`, `plan_only`, or shadow comparison.
- A failed or malformed Codex suggestion falls back to direct execution without losing the current draft, selected datasets, or conversation state.

### Task 1: Add ECharts Runtime Dependency And Contract

**Files:**

- Modify: `apps/web/package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/lib/static-page-draft.test.mjs`
- Create: `apps/web/app/lib/static-page-chart-runtime.js`
- Test: `apps/web/app/lib/static-page-draft.test.mjs`

**Status:** Completed in current baseline. The web package depends on `echarts`, draft normalization carries `chartRuntime`/`chartOptions`, unsafe option keys and script-like values are stripped, unknown runtimes fall back to deterministic rendering, and frontend draft tests cover default runtime, ECharts runtime, invalid runtime fallback, row normalization, synthesized preview options, and unsafe option stripping.

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

**Status:** Completed in current baseline. `StaticPageChartPreview` renders deterministic mini charts by default and hydrates ECharts client-side only when sanitized option/data are renderable. Module cards/final render surfaces expose compact runtime/data-quality labels, and missing ECharts data shows an explicit waiting state instead of fake bars.

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

**Status:** Completed in current baseline. `static-page-runtime` accepts only deterministic/ECharts runtimes and safe plain-JSON ECharts options; `static-page-renderer` emits deterministic HTML/SVG fallback plus safe ECharts JSON hydration islands and data-quality/fallback manifest fields; `platform-api` keeps image-prompt payload, data snapshot, final render manifest, and HTML artifact reports aligned around the same runtime/data-quality contract.

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

**Status:** Completed in current baseline; continue only with targeted correctness, validation, and regression-test hardening.

**Files:**

- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/lib/static-page-draft.test.mjs`
- Modify: `crates/static-page-runtime/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Current implementation note:** Backend draft operations now refresh the design contract even when the client sends a full edited payload. Confirmed/queued/running preview contracts are marked `stale` when the module/layout/render/mobile-order fingerprint changes, stale edits clear `previewImage` and `finalPage`, and ReAct/current-draft operations continue to apply against the persisted current draft before recording operation metadata. The web draft helper now uses the same stale rules, keeps a stale queued job visible as needing regeneration, exposes effect/final status in the planning summary, and styles the stale card correctly inside the dark assistant shell. Desktop and mobile builders now keep only module layout/editing controls; model-led whole-draft changes go through the global bottom chat composer. Static-page creation requests from chat are non-interruptive: the assistant still answers normally, and the draft entry card is appended after the messages with `进入静态页工作台`. The main chat card owns the single `效果图——生成页面` CTA after the user enters/edits the draft; it first queues/refreshes the effect image and then, after customer approval, confirms the visual contract and starts final rendering. The effect-image ready state closes the module editor and returns the user to chat; dissatisfied users reopen module editing from the same card. Current data snapshots now also attach per-module `bindingQuality`, `bindingQualityStatus`, `chartDataFit`, and `recommendedAction`; the provider-backed static-page intent runtime receives a compact `assistant_context.static_page_binding_quality` summary; and Host-Controlled ReAct current-artifact briefs now expose the same quality-only summary without leaking module body text or sample rows. The actual backend image-job and final-render routes both block weak `bindingQuality` submissions and return structured gate details with attention modules plus repair/retrieval next actions, so direct UI/API clicks and model-led actions share the same data-quality boundary even for historical confirmed previews. The frontend final-render controls now reuse the same local binding-quality gate, avoid entering optimistic background-render state for weak historical previews, and roll back if the backend still rejects a render request. The web fetch/error helper preserves `code`, `details`, status, and payload from API errors, and the preview/final failure paths can append module-level gate hints when needed. This lets model-led planning distinguish confirmed rows, inferred retrieval signals, matched-but-unmaterialized fields, and missing chart bindings instead of treating every selected field as ready.

**Current gate diagnostics note:** Static-page preview/final-render gate payloads now include per-module `gateReasons` plus aggregate `gateReasonCounts`, so the UI, ReAct observations, and support/debug logs can distinguish missing chart sample rows from weak binding status or chart-data-fit failures without parsing localized copy.

**UI freeze note:** Do not redesign the assistant shell while hardening this task. Keep the current module editor, current main-chat entry card, current single CTA, current right shelf, and current mobile pattern. Improvements should target edit correctness, stale-state handling, validation, data binding, chart options, and model-understood operations.

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

**Status:** Completed in current baseline.

**Current implementation note:** Web UI background render submission, main-workspace status card, right-shelf cancel/retry actions, ZIP handoff, data-quality chips, workflow queued/rendering state synchronization, worker manifest failure diagnostics, worker-side cancellation race protection, PostgreSQL-backed worker completion/cancel tests, static-page render retry/dead-letter workflow regression coverage, and API retry/dead-letter output-state coverage are implemented. Final render manifests and ZIP handoff now also include module-level data quality/runtime diagnostics via `data-quality-report.json`, so handoff reviewers can see each module's title, data binding, sample row count, quality status, binding quality, chart-data fit, fallback mode, and recommended action. The final-render panel shows a compact module-level quality list with localized status/runtime labels, the right-side static-page shelf surfaces the first problem/fallback modules directly on each draft card, and the safe HTML artifact layer now synthesizes read-only `static_page_data_quality_report` artifacts from rendered final-page manifests for review in the common artifact shelf.

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

**Status:** In progress. Frontend startup briefing and scope planner expose static-page/report/media capabilities, controlled continuous-action policy, recommended tool actions, selected/inferred visible-scope rules, quality-first context budget, stale static-page preview/export blockers, and compact UI intent/action chips. Backend AssistantRun scope planning now emits the same supply-policy contract, enriches visible dataset candidates from real visible documents/chunks, carries recommended tool actions into context/evidence state, preserves ReAct protocol action names separately, exposes current static-page artifact status/module count/preview stale/final-render state in the weak ReAct planning catalog without leaking module body content, summarizes the currently opened static-page artifact in provider prompts as an operable skeleton of draft id/module ids/titles/layout/data-binding/chart type instead of raw module body/data rows, promotes vague follow-up prompts such as "继续刚才那版改一下" to active static-page draft context when a draft is open while leaving unrelated ordinary chat alone, lets ReAct explicitly recall hidden local-thread conversation memory even when the original ordinary-chat scope had an empty `conversation_memory` array, expands detail-first evidence limits for static-page/report/media scopes, falls back to visible document chunks when selected-scope retrieval evidence has not been generated yet, adds model-facing supply briefs so provider prompts distinguish citable supplied items from detail targets, records a `supply_quality` report with grounded/partial/missing/not-requested status, indexed evidence count, fallback chunk count, citation locators, media context count, and model guidance, aligns retrieval-worker indexing and AssistantRun query scoring on boosted CJK phrase n-grams up to 6 characters so business phrases such as "订单延期风险" and "客户满意度" survive lexical signatures, keeps regression coverage for CJK phrase weighting plus fallback chunk ranking, and guards ordinary chat so visible datasets do not force supply. Retrieval quality still needs deeper validation on larger real customer corpora, but the local lexical baseline is now covered by realistic order/support/FAQ phrase fixtures and supply quality is now explicit for the model and runtime diagnostics.

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

### Task 7A: Direct Or Public Video PPT Extraction Flow

**Status:** In progress. This task turns the existing partial media foundation into a real assistant capability for direct video files and publicly resolvable video URLs/pages. Current baseline can parse uploaded/local media evidence, register direct video URLs, resolve simple public HTML video pages, enqueue guarded remote-media ingest, surface parsed video evidence as a safe read-only `video_extraction_summary` HTML artifact, expose shared video extraction request/state/artifact contracts, expose internal media action tool definitions, queue the dedicated video extraction workflow from assistant actions when evidence is missing, includes a thin `media-worker` for media queue stages, exposes a dedicated `video_extraction_workflow` state machine, can emit deterministic source/review artifacts, grouped final-deliverable manifests, plus a basic screenshot-based PPTX after a confirmed keep-list, exposes generated video/PPT artifacts through the persisted HTML artifact shelf with guarded download links, now emits deterministic slide-notes/quality-warning artifacts plus basic PPTX speaker-note parts, maps available transcript segments into a conservative `subtitle_page_map.json`, enriches generated `source_text.md`, `ppt_outline.md`, and `slide_notes.md` with evidence references, quality notes, selection metadata, rectangle/crop metadata, and redacted provider status, emits `video_slides.md` as a redacted final-output Markdown deck that mirrors the selected slide order, source frame names, crop status, and aligned narration, surfaces failed/skipped raw-frame extraction, generated-artifact writer failures, parse-partial evidence, provider failures, low-confidence transcript/OCR evidence, raw-frame-only slide candidates, detector-cropped slide rectangles, full-frame fallback slide rectangles, and selected-slide duplicate removal as structured deliverable warnings, classifies resolver failures as missing source / unsupported source / resolver blocked / unavailable video, maps warning states into completion follow-up next actions, renders structured background-completion follow-up plus warning-specific next-action labels in the video summary artifact, renders a redacted audit summary inside the same safe artifact, includes a status-only user notification intent for completed background extraction that is visible in the safe summary/right shelf, surfaces compact audit warning/provider-failure counts on the right-shelf video card, records a redacted `completion_audit` summary with source-resolution/provider-failure metadata on workflow-completed events, summary payloads, and durable output artifacts, emits a redacted `published_deliverable_manifest.json` that marks complete packages as immutable published version `v1` while preserving `no_host_composed_answer`, emits package-scoped `published_version_history.json` so each generated package carries a portable version history record, promotes complete packages into durable `published_video_ppt_packages` / `published_video_ppt_versions` history records with only redacted artifact pointer metadata, emits `slide_rectangles_manifest.json` so selected keep-list frames have an explicit detector/full-frame crop and dedupe contract inside the public deliverable gate, removes exact duplicate selected frame bytes and conservative visual near-duplicates before rectangle promotion/PPTX generation, can promote obvious JPEG/PNG slide rectangles with `border_background_contrast_v2`, refine dominant slide crops with `foreground_component_v1` when external foreground components would otherwise expand the crop, and can fall back to `edge_projection_v1` when non-uniform backgrounds make border-median contrast unsafe, applies detector crop boxes to generated PPTX slide images through OOXML `a:srcRect`, writes redacted native picture alt text into each PPTX slide so page/candidate/source-frame/timestamp/crop/transcript-count metadata is searchable without exposing local paths, feeds pending `video_extraction_model_completion_turn_request` records into the continue ReAct provider input as a redacted model-owned completion context, emits a durable `assistant_run.model_completion_turn_requested` dispatch request event for background video completion, has a platform-side idempotent consumer that converts those dispatch requests into the normal AssistantRun continue path, registers `assistant_run_model_completion_workflow` plus an `assistant-run-worker` that consumes the queued proactive turn through the same model/provider path, has a non-destructive Linux/deployment-target smoke entrypoint for that worker that passed on the 8-server deployment target at commit `0d5f268`, and now has the 8-server `aiv3-assistant-run-worker.service` systemd service enabled and active against the release binary. Richer editable slide reconstruction from OCR/layout remains pending.

**Temporary freeze:** On 2026-05-15 the operator froze further PPT/video development. Do not continue Task 7A or richer PPTX/OCR reconstruction unless the operator explicitly resumes this track. Continue other mainline work such as third-party integration, external bot readiness, safe HTML artifacts, static-page quality, and model-gateway/Codex executor foundations.

**Capability boundary:**

- Already exists: generic audio/video upload ingestion, direct video URL registration, guarded simple public-page video URL resolution, guarded remote-media ingest, media metadata/transcript/scene/keyframe evidence surfaces, media detail API, timestamp-aware retrieval/static-page supply, model-visible ReAct action contracts for URL resolution and video PPT/transcript extraction, startup briefing policy that separates model-request actions from the full `resolve -> register -> extract` controlled pipeline, scope-planner hints for uploaded-video extraction and public/direct video source resolution without forcing dataset retrieval, safe HTML artifact contracts/templates for displaying extraction summaries, shared video extraction request/source/asset/artifact/state contracts, internal media action tool definitions, assistant-action queuing into the dedicated video extraction workflow when parsed evidence is missing, a thin `media-worker` that advances media queue workflow stages and records a `raw_frames` extraction plan, optional local-file FFmpeg frame extraction behind an explicit env flag, local `frame_manifest.json` output for successful extraction runs, deterministic local `transcript.txt` / `source_text.md` / `ppt_outline.md` / `timestamp_map.json` writer from parsed media evidence, conservative `slide_candidates_manifest.json` plus `contact_sheet_plan.json`, `raw_contact_sheet.html`, `ppt_keep_list_template.json`, `selected_slides_manifest.json`, `slide_rectangles_manifest.json`, `slide_notes.md`, `video_slides.md`, `subtitle_page_map.json`, `video_slides_screenshot_based.pptx`, `final_deliverables_manifest.json`, `published_deliverable_manifest.json`, `published_version_history.json`, and `pptx_build_plan.json` from raw frames and selected keep-lists, raw frame manifests and generated files promoted into the standard video extraction artifact refs, resolver observations with explicit `failure_kind` / `failure_next_action` for missing source, unsupported source, resolver-blocked, and unavailable-video cases, explicit `deliverable_status` and quality warnings in worker output and safe summaries, structured `frame_extraction_failed` / `frame_extraction_skipped` / `generated_artifacts_failed` / `parse_partial` / `provider_failure` / `low_confidence_transcript` / `subtitle_ocr_low_confidence` / `no_slide_rectangle_found` / `full_frame_rectangle_fallback` / `selected_slide_duplicates_removed` / `published_version_missing` deliverable warnings when raw frame capture, generated-artifact writing, missing parse evidence, provider capability, transcript confidence, OCR confidence, missing selected slide rectangles, review-required full-frame fallback crops, selected duplicate frames, or missing published-version metadata need review, warning-specific completion follow-up actions for retrying frame extraction, providing local media/parsed frames, retrying the generated-artifact writer, attaching/parsing transcript evidence, generating a contact sheet, reviewing contact sheets, slide rectangles, or selected-slide dedupe manifests, filling the keep-list, rerunning/refreshing video parsing, checking provider configuration, reviewing low-confidence transcript, rerunning/reviewing subtitle OCR, or persisting the published version, readable warning-specific next-action labels in the existing safe video summary artifact, AssistantRun event handoff plus persisted read-only `video_extraction_summary` HTML artifact after background completion, generated-file visibility inside the safe video extraction summary artifact, guarded generated-file download URLs for video summaries, right-shelf discovery for video/PPT HTML artifacts, basic PPTX speaker-note XML parts with source frame metadata and available transcript pre-page assignment, structured `completion_follow_up` metadata in AssistantRun events/output artifacts/HTML summaries, a first immutable published-version manifest plus package-scoped published-version history for complete final packages, durable database-backed published video/PPT package and version records with redacted artifact pointer metadata, a selected keep-list to detector/full-frame rectangle/dedupe public manifest contract, exact and conservative visual selected-frame dedupe before rectangle promotion, conservative JPEG/PNG background-contrast and edge-projection rectangle detection, detector crop application in generated PPTX slide XML, and a dedicated video extraction workflow stub.
- Missing: stronger visual rectangle detection/crop replacement beyond the current conservative contrast/edge heuristics, and richer native PPTX rendering.
- Explicitly out of scope for this slice: site-specific login adapters, QR login, cookies/session reuse, login-gated sites, paywalled/private pages, and browser recording bypass flows.
- Required assistant behavior: the assistant must know this capability from the startup briefing/action catalog. When the user uploads a video or provides a direct/publicly resolvable video URL/page and asks to extract PPT/original text, the model should answer in its own words and request the V3 action. If the model/provider fails, V3 must show failure and must not compose a fake assistant answer.

**Implementation reference:** The local Codex skill `wechat-video-ppt-extract` is usable as the fixed post-video SOP for slide reconstruction. Treat the name as historical: in V3 it should be generalized to uploaded/direct/public videos, not WeChat-specific automation. The reliable reusable portion is: frame capture or frame import -> contact sheet -> light PPT rectangle extraction -> conservative dedupe -> manual/model-assisted keep-list -> screenshot-based PPTX with manifest/source notes. The existing helper script does not directly ingest an `.mp4`; if V3 already has a video file, first extract frames with FFmpeg/media worker into a `raw_frames`-style directory, then run the rectangle extraction/build steps and skip any recording step. The updated skill also defines an optional subtitle-overlay pass: OCR subtitle bands from original video frames, assign subtitles to pages using the pre-page rule, place text in a consistent box outside the PPT image area, and save transcript text plus JSON page mapping for later correction.

**Target flow:**

1. User uploads a video file, provides a direct video URL, or provides a public page where V3 can safely resolve a video asset URL.
2. AssistantRun receives product/system briefing that explicitly lists `media.resolve_video_url`, `media.register_video_asset`, and `media.extract_ppt_transcript` as available V3-controlled actions.
3. The model decides whether to ask V3 to parse the uploaded media directly or resolve/register a public video URL. It should not claim the video was accessed until V3 returns a successful observation.
4. V3 validates the action, registers the uploaded/resolved video as a user/session-scoped document or asset, and returns a concise progress step in the existing chat/right-shelf surfaces.
5. V3 enqueues background media parsing. Foreground chat remains responsive.
6. Media parsing extracts audio transcript, OCR/keyframes/scenes, timestamps, and provider evidence through MiniMax/local tools where available.
7. A PPT extraction worker groups keyframes/transcript into slide candidates, extracts original spoken text, generates a slide outline and optionally a PPTX/Markdown artifact, and records source timestamps for every page.
8. The assistant receives the parsed evidence and returns the final model-authored summary, original transcript, and links to the PPT/transcript artifacts.

**Files:**

- Modify: `apps/web/app/lib/assistant-startup-briefing.js`
- Modify: `apps/web/app/lib/scope-planner.js`
- Modify: `apps/web/app/lib/html-artifact-manifest.js`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/react_agent_catalog.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/workflow-definitions/src/lib.rs`
- Modify: `crates/ingest-worker/src/lib.rs`
- Create or modify: `crates/media-worker/src/lib.rs`
- Create or modify: `crates/media-worker/src/main.rs`
- Create or modify: `crates/assistant-run-worker/src/main.rs`
- Create or modify: `crates/static-page-worker/src/lib.rs` only if extracted PPT artifacts reuse static-page/report render helpers.
- Create migration if durable video extraction task/session rows are needed: `crates/storage/migrations/00xx_video_extraction.sql`

**Implementation steps:**

1. Completed current slice: add shared contracts for `VideoExtractionRequest`, `VideoExtractionState`, video source refs, resolved asset refs, and extraction artifact refs.
2. Completed current slice: add V3 action contracts: `media.resolve_video_url`, `media.register_video_asset`, and `media.extract_ppt_transcript`.
3. Completed current slice: add assistant startup-briefing policy that tells the model this capability exists, when to request it, the full `media.resolve_video_url -> media.register_video_asset -> media.extract_ppt_transcript` controlled pipeline, and the hard rule: do not say the video was accessed until the action observation says it was captured.
4. Completed current slice: add scope-planner/media intent hints for uploaded video, direct video URLs, and public video pages so the model sees the relevant action options without forcing dataset retrieval or a deterministic host answer.
5. Completed current slice: implement a public-page resolver that accepts only safe HTTP(S) URLs, rejects login-required/private pages/private hosts/redirects, detects direct media URLs and simple HTML `<video>`/`<source>`/OpenGraph/Twitter video sources, and returns a bounded metadata/error observation.
6. Completed current slice: implement the video extraction workflow definition with states: `resolving_source`, `registered`, `parsing`, `extracting_ppt`, `completed`, `failed`, and `unsupported_source`.
7. Completed current slice: assistant extraction requests now queue `video_extraction_workflow` for selected uploaded/resolved video documents when parsed evidence is missing; `media-worker` advances `resolve_video_source`, `register_video_asset`, and `extract_video_ppt`; `ingest-worker` can pick `parse_video_media` from the shared `ingest` queue. Remaining work is the final artifact writer.
8. Completed current slice: `media-worker` emits a disabled-by-default FFmpeg/raw_frames extraction plan using `video-extraction-{document_id}/raw_frames`, `frame_%06d.jpg`, interval `0.15`, and contact-sheet source `raw_frames`, matching the `wechat-video-ppt-extract` SOP. When `MEDIA_FRAME_EXTRACTION_ENABLED=true`, it executes FFmpeg only for local files or `PLATFORM_LOCAL_OBJECT_ROOT` files, writes `frame_manifest.json`, and returns a frame manifest plus `frame_manifest` artifact ref with its file path in workflow output. It writes deterministic local `transcript.txt`, `source_text.md`, `ppt_outline.md`, `timestamp_map.json`, `slide_candidates_manifest.json`, `contact_sheet_plan.json`, `raw_contact_sheet.html`, `ppt_keep_list_template.json`, `selected_slides_manifest.json`, `slide_rectangles_manifest.json`, `pptx_build_plan.json`, `final_deliverables_manifest.json`, `published_deliverable_manifest.json`, and `extraction_artifacts_manifest.json` when parsed media evidence or raw frames exist. If a keep-list already contains selected candidate numbers, it preserves that keep-list, writes a normalized selected slide manifest, removes exact duplicate selected frame bytes, promotes remaining JPEG/PNG frames through conservative background-contrast rectangle detection when possible, falls back to full-frame rectangles when detection is not safe, applies detector crop boxes to generated PPTX image fills through `a:srcRect`, builds a basic screenshot-based `video_slides_screenshot_based.pptx` with one selected frame per slide, marks the PPTX build plan `completed`, writes grouped final and published-deliverable manifests, and exposes `deliverable_status=final_pptx_ready` in output/HTML summaries. It also appends an AssistantRun event and persists a read-only `video_extraction_summary` HTML artifact when background extraction completes.
9. Completed current slice: generated video/PPT files in `video_extraction_summary.payload.generated_artifacts.files` can be discovered in the right-side generated project shelf and downloaded through a V3-validated route that only serves files inside the generated artifact workspace. This closes the first final-deliverable lifecycle gap for persisted discovery plus download and now marks complete generated packages with an immutable published-version manifest; durable history/object-store promotion remains separate.
10. Completed current slice: emit `slide_notes.md`, add deterministic quality warnings into `deliverable_status`, render those warnings in the safe video extraction summary, and write basic PPTX notes-slide relationships/parts containing source frame and candidate metadata.
11. Completed current slice: when transcript segments and a confirmed keep-list exist, generate `subtitle_page_map.json` using the conservative pre-page assignment rule, include the mapped transcript in `slide_notes.md`, and write the same evidence into PPTX speaker-note parts. Missing transcript remains explicit, and customer-facing source text, timestamp maps, review JSONs, slide notes, PPTX notes, and downloadable deliverable manifests redact internal frame paths, URLs, tokens, and provider secrets while retaining source frame file names, manifest file names, timestamps, and candidate ids.
12. Completed current slice: feed generated video/PPT artifacts back into durable AssistantRun `output_artifacts`, dedupe by video document, and preserve links to the persisted `video_extraction_summary` artifact.
13. Completed current slice: add structured `completion_follow_up` metadata to AssistantRun workflow-completed events, output artifacts, and the safe video summary payload; the safe HTML summary now displays ready files and next actions without host-composing a final assistant answer.
14. Partially completed current slice: reuse the skill SOP for deeper slide reconstruction. V3 now generates a contact sheet, preserves a candidate manifest, accepts selected keep-list entries, emits review-required full-frame slide rectangles, and builds a screenshot-based PPTX with speaker-note/source metadata. Remaining work is replacing the fallback rectangles with real visual crops and improving slide rendering beyond screenshot pages.
15. Partially completed current slice: generated `source_text.md` and `ppt_outline.md` now include transcript/scene/OCR evidence refs, confidence/source labels, explicit quality notes for missing/low-confidence evidence, provider capability failures, frame-extraction skips/failures, and redacted provider capability status; `slide_notes.md` now includes deck-level selection/dedupe/rectangle summaries plus page-level contact-sheet anchors, transcript windows, rectangle status, crop boxes, and review flags. Richer transcript segmentation, keyframe clustering, OCR slide text promotion, and PPTX rendering remain pending.
16. Partially completed current slice: completion follow-up now carries a status-only `user_notification` payload, and the safe video summary plus right-shelf card consume it without host-composing an assistant answer; automatic proactive background-turn dispatch remains pending.
17. Completed current failure-handling slice: surface raw frame extraction `failed` / `skipped` states, generated-artifact writer failures, parse-partial evidence, provider failures, low-confidence transcript/OCR evidence, raw-frame-only slide candidates without selected rectangles, and review-required full-frame fallback rectangles as structured deliverable warnings with reason/count/missing-evidence/provider/candidate metadata; classify resolver failures for unsupported source, unavailable video, resolver blocked, and missing source; and map those plus core review warnings into completion follow-up next actions.
18. Partially completed current slice: render warning-specific video extraction completion actions and redacted completion-audit rows as readable labels inside the existing safe HTML artifact/right-shelf summary surface, including the contact-sheet/rectangle-extraction review action, and surface compact audit warning/provider-failure counts on the existing right-shelf video card. Remaining UI artifact work must stay within the existing HTML artifact/right-shelf/main-chat surfaces and must not add another chat input, popup workbench, or global panel.
19. Partially completed current slice: video extraction workflow-completed events, safe summary payloads, and durable output artifacts now include a redacted `completion_audit` with output/deliverable states, artifact kinds, warning codes, source-resolution status flags, provider-failure labels, and provider-failure counts, without copying cookies, private URLs, raw tokens, provider keys, raw provider payloads, or local source paths.
20. Completed current slice: `slide_candidates_manifest.json` now attaches timestamp-nearby transcript/scene/OCR evidence references to each raw-frame candidate, records matched-vs-raw-frame-only evidence status plus aggregate ref counts, and sanitizes source/provider/locator labels before writing candidate review metadata. This gives the actual generated-page candidate path enough evidence traceability for later data-quality gates while stronger visual rectangle extraction remains pending.
21. Completed current slice at commit `e8e5538`: video/PPT deliverable validation now reads the PPTX ZIP central directory and requires core OOXML entries (`[Content_Types].xml`, root relationships, `ppt/presentation.xml`, slide, slide rels, and notes slide), so a ZIP magic-only placeholder can no longer pass the public deliverable smoke. Local media-worker controlled sample, Node deliverable tests, jump-host self-test, and 8-server Node deliverable tests passed.
22. Completed current slice: complete video/PPT packages now emit `published_deliverable_manifest.json` with `manifest_type=v3.video_ppt_published_deliverable.v1`, `lifecycle_state=published_version_ready`, `immutable_version=true`, `version_no=1`, redacted file entries, and no host-composed answer. Output artifacts, completion follow-up metadata, audit group counts, the Node deliverable validator, and the jump-host self-test fixture all treat this as part of the public deliverable contract.
23. Completed current slice: selected keep-list frames now emit `slide_rectangles_manifest.json` with `rectangle_extraction_status=promoted_full_frame_fallback`, `rectangle_extraction_mode=full_frame_fallback`, order-preserving de-duplicated candidate indices, relative full-frame crop boxes, and `review_required=true`. Worker output, final/published/extraction manifests, completion follow-up actions, the Node deliverable validator, and the jump-host self-test fixture now treat this as part of the public video/PPT deliverable gate.
24. Completed current slice: selected keep-list frames are now exact-content de-duplicated before rectangle promotion and PPTX generation. `selected_slides_manifest.json` and `slide_rectangles_manifest.json` record `dedupe_status=exact_frame_content_deduped`, `deduped_candidate_count`, and `rejected_duplicate_candidates` when two selected frame files have identical bytes.
25. Completed current slice: selected JPEG/PNG frames can pass through a conservative background-contrast detector and promote obvious non-background slide rectangles into relative crop boxes. Detector crops are marked `promoted_detector_crop` and still require review; ambiguous, tiny, full-frame, or undecodable images remain on the safe full-frame fallback path. The public validator accepts detector crops and fallback crops while still rejecting invalid relative boxes or missing review flags.
26. Completed current slice: generated PPTX slide XML now applies detector crop boxes with DrawingML `a:srcRect`, so detected rectangles affect the actual PPTX image fill instead of only appearing in manifests. Full-frame fallback crops deliberately omit `srcRect` and keep the previous whole-frame rendering behavior.
27. Completed current slice: selected keep-list frames now also run a conservative visual-similarity dedupe for JPEG/PNG frames before rectangle promotion and PPTX generation. The worker writes `visual_fingerprint`, `visual_duplicate_count`, `visual_similarity_score`, matched candidate metadata, and `dedupe_status=visual_similarity_deduped` or `exact_and_visual_similarity_deduped` when near-duplicate frames are rejected, while keeping exact-byte-only behavior for undecodable frame files.
28. Completed current slice: selected-slide duplicate removal is now surfaced as a structured `selected_slide_duplicates_removed` deliverable warning with exact/visual duplicate counts, mapped to a `review_slide_dedupe_manifest` follow-up action, rendered with a readable summary label, and echoed into `slide_notes.md` quality warnings.
29. Completed current slice: slide rectangle detection now uses `border_background_contrast_v2`, sampling the full image border and taking a median RGB background instead of trusting the top-left pixel. This keeps the detector conservative and review-required while handling small edge noise/logo pixels that previously forced full-frame fallback. The public validator accepts both `border_background_contrast_v2` and older `simple_background_contrast_v1` manifests.
30. Completed current slice: `slide_notes.md` is now a richer Markdown review artifact. It starts with a deck summary, then each selected slide records contact-sheet anchor, transcript assignment window, rectangle extraction status/mode/source, relative crop box, and review-required flag while keeping local paths redacted.
31. Completed current slice: complete video/PPT packages now emit `published_version_history.json` with `manifest_type=v3.video_ppt_published_version_history.v1`, package-scoped history status, latest immutable `v1`, a pointer to `published_deliverable_manifest.json`, and redacted file coverage. The media worker, public deliverable validator, jump-host smoke doc, and right-shelf labels now treat this as part of the published package contract while durable history promotion remains pending.
32. Completed current slice: slide rectangle detection now has a conservative `edge_projection_v1` fallback after `border_background_contrast_v2`. It detects strong rectangular edge projections when noisy or gradient backgrounds make the median-background crop unsafe, records `raw_frame_edge_projection` detector metadata in `slide_rectangles_manifest.json`, and keeps every promoted crop review-required. The public validator and smoke docs now accept this mode.
33. Completed current slice: complete video/PPT packages now emit `video_slides.md` as a redacted final-output Markdown deck alongside `video_slides_screenshot_based.pptx`. The media worker writes slide order, source frame file names, contact-sheet anchors, crop status, relative crop boxes, and available aligned narration from `selected_slides_manifest.json`; final/published/extraction manifests, completion follow-up actions, the Node validator, and jump-host smoke fixture treat it as part of the required public deliverable contract.
34. Completed current slice: the model startup briefing, safe video summary renderer, and right-shelf download labels now recognize `video_slides_markdown` / `video_slides.md` as a first-class video/PPT deliverable. Operators can see Markdown deck readiness, download priority, generated-file labels, and follow-up review actions without inferring it from manifests.
35. Completed current slice: background video extraction completion now emits a structured `video_extraction_model_completion_turn_request` inside `completion_follow_up`, durable output artifacts, and the workflow-completed event payload. The request carries ready file kinds, warning codes, missing required file kinds, artifact IDs, and a strict answer contract so the next user-visible update can be model-authored without host-composing a fake final answer; actual model-turn executor wiring remains a separate closure.
36. Completed current slice: the safe video summary artifact and right-shelf video card now surface the model completion turn request instead of hiding it behind a generic follow-up flag. The summary shows model ownership, ready file kinds, warning/missing signals, and answer boundaries; the right shelf marks video cards as waiting for a model-authored completion response.
37. Completed current slice: assistant-run continue ReAct input now reads pending video model completion requests from durable output artifacts and passes only a redacted whitelist into the model context: source artifact id/title/document id, completion status, ready file kinds, HTML artifact ids, warning/missing signals, primary next action, and the answer contract. The prompt explicitly requires `final_answer` to be model-authored from this context/observations, forbids claiming missing files are ready, and blocks private paths, internal URLs, cookies, tokens, provider payloads, or login/recording bypass requests.
38. Completed current slice: media-worker now emits a durable `assistant_run.model_completion_turn_requested` event and mirrors the same dispatch request into video output artifacts when background video extraction has a required model completion request. The dispatch request is queued/idempotent, targets `continue_assistant_run`, carries a one-step continue prompt plus the existing model completion context/answer contract, and explicitly preserves `no_host_composed_answer` and private-path/internal-URL redaction. The remaining proactive closure is an assistant-run worker/consumer that executes those queued dispatch requests through the normal model/provider path.
39. Completed current slice: platform-api now exposes an idempotent `consume_assistant_run_model_completion_turn_dispatch` core consumer for queued model-completion dispatch requests. It validates the dispatch kind/target/idempotency key, reuses the same AssistantRun continue execution/persistence path as `/v1/assistant-runs/{run_id}/continue`, appends `assistant_run.model_completion_turn_consuming` and `assistant_run.model_completion_turn_consumed` events, and returns `already_consumed` on duplicate idempotency keys instead of generating another assistant message. The HTTP continue handler was refactored to call the same loaded-run helper, keeping manual and background completion turns on one model/provider path.
40. Completed current slice: video background completion now creates `assistant_run_model_completion_workflow` executions on the `assistant_run` queue when a dispatch request is present, records `assistant_run.model_completion_turn_enqueued`, and avoids duplicate scheduling for the same idempotency key once an enqueued/consumed event exists. The new `assistant-run-worker` claims `consume_model_completion_turn` tasks, validates the workflow kind, calls the same platform-side consumer through `AppState`, advances the workflow to `assistant_run_model_completion_consumed`, and marks duplicate dispatches as normal idempotent completions instead of composing host-side answers.
41. Completed current slice: added `scripts/run-assistant-run-worker-smoke.sh` as a non-destructive Linux/deployment-target smoke for the new worker. It checks `assistant-run-worker` build/tests, workflow queue registration, domain-model workflow-kind roundtrip, and the media dispatch redaction/idempotency contract, writes JSON/Markdown reports under `target/assistant-run-worker-smoke`, and gates the DB-backed consumer test behind `ASSISTANT_RUN_WORKER_SMOKE_DATABASE_TEST=true` plus a disposable test database guard. README, scripts, and validation docs now expose the worker run command and smoke entrypoint.
42. Completed current slice: pulled commit `0d5f268` onto the 8-server deployment target at `/srv/aiv3/repo` and ran `bash scripts/run-assistant-run-worker-smoke.sh` there successfully. The target report is recorded in `docs/validation/assistant-run-worker-smoke.md`; the DB-backed consumer check remains intentionally opt-in for a disposable test database.
43. Completed current slice: built the deployment release binary with `CC=clang CXX=clang++ cargo build -p assistant-run-worker --release` on 8-server, registered `/etc/systemd/system/aiv3-assistant-run-worker.service`, enabled and started it, and confirmed it is active with the expected `assistant_run / consume_model_completion_turn` queue, NATS wake path, and database-polling fallback. The target host's default GCC 10 release-build blocker is documented in the smoke record.
44. Completed current slice: complete video/PPT packages now promote redacted durable history records into `published_video_ppt_packages` and `published_video_ppt_versions`. The stored manifest uses `v3.video_ppt_durable_published_version.v1`, keeps only artifact pointer/file metadata with paths redacted, appends `video_extraction.published_version_persisted`, and links the durable version reference back into the AssistantRun output artifact.
45. Completed current slice: pulled durable video/PPT published-version history commit `5707747` onto the 8-server deployment target at `/srv/aiv3/repo`, ran the storage migration-order test and media-worker durable published-version tests there, built `platform-api` and `media-worker` release binaries with `CC=clang CXX=clang++`, restarted `aiv3-platform-api.service` and `aiv3-media-worker.service`, confirmed both services active, and verified `published_video_ppt_packages` / `published_video_ppt_versions` exist in the target database.
46. Completed current slice: slide rectangle detection now has a conservative `foreground_component_v1` refinement after border-median foreground projection. When external foreground components such as status strips or overlays would expand the crop, the worker promotes only the dominant connected slide component if it clearly owns the foreground signal; the manifest records `raw_frame_foreground_component`, keeps every crop review-required, and preserves the older background/edge detectors for simpler frames.
47. Completed current slice: pulled foreground-component crop commit `afb5a4d` onto the 8-server deployment target, ran `cargo test -p media-worker slide_rectangle --lib`, built the `media-worker` release binary with `CC=clang CXX=clang++`, restarted `aiv3-media-worker.service`, and confirmed the service active with NATS connection and media polling logs.
48. Completed current slice: screenshot-based PPTX slides now write redacted native picture alt text with slide number, candidate number, source frame name, timestamp, crop mode, transcript segment count, and `paths redacted`; `pptx_build_plan.json` records this native metadata policy so generated decks are easier to inspect/search without exposing local artifact paths.
49. Completed current slice: pulled native PPTX metadata commit `5a3a96b` onto the 8-server deployment target, ran the PPTX slide XML metadata regression test, built the `media-worker` release binary with `CC=clang CXX=clang++`, restarted `aiv3-media-worker.service`, and confirmed it active.

**Tests:**

1. `cargo test -p contracts video_extraction`
2. `cargo test -p assistant-runtime video_extraction`
3. `cargo test -p platform-api video_extraction`
4. `cargo test -p workflow-definitions video_extraction`
5. `cargo test -p ingest-worker media`
6. `cargo test -p media-worker` if the crate exists
7. `cargo test -p assistant-run-worker`
8. `bash scripts/run-assistant-run-worker-smoke.sh`
9. `node --test apps/web/app/lib/html-artifact-manifest.test.mjs`
10. `pnpm --filter @ai-data-platform-v3/web build`
11. Resolver smoke only after safe stubs pass: direct MP4 fixture, simple public HTML `<video>` fixture, unsupported/login-required fixture, and blocked/private URL fixture.

**Acceptance:**

- The assistant knows the direct/public video PPT extraction capability through model-facing briefing/action contracts, not through a host-composed answer.
- Uploaded video files and direct/publicly resolvable video URLs can enter the same background parsing path.
- Unsupported/login-gated/private sources are rejected with a clear observation instead of QR/login/recording workarounds.
- Resolved video is registered as a user/session-scoped V3 document or asset and then parsed by background workers.
- Final deliverables include original transcript text, optional subtitle/page map, screenshot-based PPTX or PPT/slide outline, Markdown artifact, source timestamps, parse confidence, candidate contact sheet, and missing-data warnings.
- If MiniMax/model/provider fails, V3 reports the failure and preserves the user message; it does not fabricate the answer or pretend the video was accessed.
- No login-gated acquisition, QR login, cookies, paywall bypass, or browser recording fallback is part of this slice.

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

**Status:** In progress. Frontend/contract/backend listing, durable manifest storage, and action-submission slices are implemented: web-side manifest normalization rejects unsafe templates, scripts, remote URLs, event handlers, form posts, provider tokens, queue secrets, and secret-like payload strings; the new `HtmlArtifactViewer` renders trusted templates in sandboxed iframes; the right shelf can list/open HTML artifacts; shared Rust contracts now include `HtmlArtifactManifestView`; Codex Host dry-run/plan-only/codex-exec outputs can attach a read-only `codex_execution_report` manifest without embedding raw process logs; storage has a standalone `html_artifacts` table/repository; `/v1/html-artifacts` now prefers persisted artifacts by run id or local thread id and opportunistically backfills older AssistantRun event manifests; backend listing synthesizes interactive `static_page_planning_handoff` artifacts for visible static-page drafts, read-only `static_page_data_quality_report` artifacts from rendered final-page manifests, and read-only `code_review_summary` artifacts from structured AssistantRun output/events with unsafe strings redacted before persistence; `/v1/html-artifacts?report_plan_id=...` now validates report-plan visibility, synthesizes and persists read-only `report_render_summary` artifacts for report render outputs, and the web right shelf also keeps a local fallback summary for the selected report plan's render outputs; ReAct video PPT extraction can now attach read-only `video_extraction_summary` artifacts from sanitized transcript/scene/keyframe OCR evidence; `/v1/html-artifacts/{artifact_id}/events` accepts only V3-validated `html_artifact.patch` or `html_artifact.action_intent` submissions for non-read-only backend artifacts and records them as AssistantRun events; static-page planning handoff patches with owner scope `static_page_draft` execute through a restricted JSON Patch -> static-page operation translator for module title/content/data binding/visualization/layout, `styleDirection`, and `mobileOrder`; action-intent handoffs now collect a natural-language prompt in the sandbox and apply it through the existing static-page intent interpreter; the frontend refreshes the target static-page draft after a successful artifact event. Broader non-static-page product persistence beyond report render summaries, code review summaries, and video evidence summaries is still pending.

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
cargo test -p llm-gateway
cargo test -p assistant-runtime
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
cargo test -p llm-gateway
cargo test -p assistant-runtime codex
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
Keep the current overall UI shell stable. Do not redesign the left dataset rail, top toolbar, home-only composer, right output shelf, static-page entry card, or single `效果图——生成页面` CTA unless fixing a blocking bug.
Product mainline is static-page generation quality: module editing correctness, data snapshots, ECharts advanced runtime, Cloudflare/Codex preview, durable final render/export, planning quality, parsing quality, retrieval/context quality, and generation quality.
The model-gateway/Codex conversation executor is now a core architecture track, not a distant sidecar. Build toward routing normal assistant turns through Codex behind feature flags and shadow/dry-run comparison, with V3 supplying context/evidence/tool contracts and validating every requested action.
Gateway model profiles must support multiple provider APIs such as GPT-family and MiniMax-compatible providers through explicit capability manifests, redacted credentials, and fallback policy.
Safe HTML artifacts are a shared review/control surface for Codex reports, planning handoffs, and lightweight JSON-patch editors; do not confuse them with final customer static-page delivery.
Video/PPT extraction is temporarily frozen as of 2026-05-15 unless the operator explicitly resumes it. Do not continue richer PPTX/OCR reconstruction or related media work while frozen.
Do not resume account expansion unless fixing a security/access regression.
Do not run real Codex execution on the local developer workstation; Codex real validation is only for the jump host or later Mac host.
Continue with backend/static-page quality work while also starting Task 0A model-gateway/Codex executor foundation without changing the visible static-page workflow.
```
