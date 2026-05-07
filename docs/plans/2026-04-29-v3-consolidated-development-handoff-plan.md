# AI Data Platform V3 Consolidated Development Handoff Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Consolidate the V3 assistant, static-page generation, document/media parsing, report outputs, separated memory, Codex Host execution-kernel direction, and optional OpenClaw extension into one execution plan that a fresh development thread can continue from without re-reading the whole project history.

**Architecture:** V3 remains a host-controlled data platform: PostgreSQL is the source of truth, the assistant supplies model context rather than composing answers locally, and all data visibility, AssistantRun state, memory-space policy, draft state, queue state, and output artifacts are owned by V3. Static-page generation remains the core product loop. Codex Mac Host is now the preferred execution-kernel direction behind V3 validation and audit. OpenClaw has completed its first optional provider/stub pass and stays as a removable sidecar, not the main execution route.

**Tech Stack:** Next.js 16 / React 19 in `apps/web`; Rust crates including `platform-api`, `llm-gateway`, `static-page-runtime`, `static-page-worker`, `static-page-renderer`, `ingest-worker`, `retrieval-worker`, `memory-worker`, `document-vlm-runtime`; PostgreSQL 17.9 target; Cloudflare/Codex image queue endpoint; optional OpenClaw Gateway `/v1/responses` and `/v1/chat/completions`; local-first parsers plus configured MiniMax VLM/media capability probes.

---

## Source Plans

Use this document as the active execution entry point.

Detailed source documents remain valid as references:

- `docs/plans/2026-04-27-static-page-generation-studio-plan.md`: detailed static-page, assistant shell, ingestion, visibility, AssistantRun, and renderer plan/history.
- `docs/plans/2026-04-29-openclaw-extension-adapter-plan.md`: detailed optional OpenClaw adapter plan.
- `docs/plans/2026-04-29-v3-react-agent-refinement-plan.md`: detailed ReAct refinement plan comparing the Java reference, the original TS contract, and current V3 AssistantRun implementation.
- `docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md`: active plan for Codex Mac Host as future execution kernel and first-class separated memory spaces.
- `docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md`: detailed boundary plan for Codex host integration versus V3-owned model gateway/proxy.
- `docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md`: active architecture boundary plan aligning V3's original control/integration/workflow/worker planes with Codex Host and future model proxy.
- `docs/plans/2026-05-07-v3-email-account-auth-plan.md`: account/email authentication plan that upgrades local-key visibility into email-bound user ownership and verification-code login.
- `docs/plans/2026-04-23-v3-development-plan.md`: older high-level V3 phase baseline.
- `docs/plans/2026-04-25-static-page-visual-workbench-v3-plan.md`: older visual-workbench plan; keep only as Cloudflare image-provider and visual-contract reference. Do not revive the separate popup/workbench direction unless explicitly requested.

## Current Baseline

Latest committed code baseline:

```text
Current branch includes the completed first ReAct refinement pass through gated OpenClaw bridge stubs; use `git log --oneline` for the exact latest commit.
```

Plan files in this consolidation slice:

```text
docs/plans/2026-04-27-static-page-generation-studio-plan.md
docs/plans/2026-04-29-openclaw-extension-adapter-plan.md
docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md
docs/plans/2026-04-29-v3-react-agent-refinement-plan.md
docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md
docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md
docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md
docs/plans/2026-05-07-v3-email-account-auth-plan.md
```

Recent verified capabilities:

- Original-assistant-style web shell exists with left dataset rail, main chat/workspace area, right draft/output shelf, mobile support, upload entry, and one static-page generation action.
- Upload path saves real local files, classifies them, registers documents, and starts ingest workflows.
- Ingest worker has local text, OOXML, PDF, OCR fallback, MiniMax document VLM slice, and first media upload slice.
- Dataset visibility foundation exists with local key binding, public/private dataset filtering, and selected-scope semantics.
- AssistantRun persistence exists for ordinary chat, hidden conversation memory, deterministic scope planning, evidence supply, continue API, static-page draft creation, and output artifacts.
- Account hardening now scopes dataset outputs, chat sessions, workflow-linked runtime details, document/RAG evidence, AssistantRun reads/events/continue, conversation-memory recall, report/static-page artifacts, and memory-directory aggregates by the active user and visible document set.
- AssistantRun ReAct refinement now includes typed contract parsing, weak planning catalog, extracted tool registry, protocol-repair matrix, report handoff, bounded document-detail reads, redacted trace summaries, frontend safe progress display, and gated OpenClaw memory/readonly bridge stubs.
- OpenClaw optional provider and gated ReAct stubs have completed their first pass; do not continue OpenClaw as the main execution-kernel route.
- Codex Host first queue bridge exists: `codex_host_task` is disabled-by-default and allowlisted, `codex_host_task_workflow` enqueues `codex_host/run_codex_host_task`, and `crates/codex-host-agent` can complete that queue in dry-run mode or build a redacted plan-only command summary without launching local Codex.
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
- Memory must be separated by explicit scope. Conversation, project, task, dataset, and system memory cannot leak across scopes unless V3 selects and audits that scope.
- Codex Host tasks must get isolated task memory. Task memory is not recalled into ordinary conversation unless V3 explicitly promotes a safe summary.
- Email account auth upgrades the current local-key model. Email proves account ownership, local key remains an access factor, and verification-code login can restore account access without ever emailing or storing the raw key.
- Datasets, robots, conversations, memory spaces, reports, static pages, and artifacts should follow `user_id` ownership while public datasets remain visible to unsigned users.

## 2026-05-03 Direction Update: Codex Kernel And Separated Memory

OpenClaw work has already completed a useful first optional-extension version:

- `llm-gateway` has an optional OpenClaw provider.
- AssistantRun and static-page runtime can route through OpenClaw provider configuration.
- ReAct has gated OpenClaw memory/readonly execution stubs.

This is now considered enough for the OpenClaw line unless a specific compatibility bug appears. The main execution-kernel route should move to Codex Mac Host:

```text
V3 Web/API -> AssistantRun/ReAct -> V3 memory-space policy -> Workflow/task audit -> Codex Mac Host -> artifacts/logs back to V3
```

The separated memory plan is now the next architecture foundation before any real Codex host daemon:

- Default browser conversations get isolated conversation memory spaces.
- Project memory is explicit and selected, not inferred silently.
- Dataset memory remains governed by selected scope and private/public visibility.
- Codex host tasks get task memory spaces that do not bleed back into conversation memory by default.
- Runtime inspect must show memory-space decisions and rejected cross-scope access.

Use `docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md` as the active implementation plan for this direction.

Use `docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md` for the separate model gateway/proxy boundary. Codex CLI/app-server/exec-server are execution/runtime surfaces, not the V3 model gateway. V3 must own provider routing, credentials, redaction, model capability policy, and audit through `llm-gateway` or a future internal `model-proxy`.

## 2026-05-07 Architecture Alignment: V3 Planes, Codex Host, And Model Proxy

The rough product split remains useful:

- Model proxy.
- Codex execution host.
- V3 permissions, datasets, memory, workflows, and artifacts.

But the implementation must follow V3's original architecture planes, not a new three-service rewrite. Treat the system as six logical modules:

- Experience UI: web shell, main workspace, mobile interaction, and right-side draft/output shelf.
- V3 Control Plane: `platform-api`, auth/scope, local-key visibility, AssistantRun state, draft/output ownership, and runtime inspect.
- Integration Plane: `llm-gateway`, future internal `model-proxy`, `tool-registry`, `mcp-gateway`, and `prompt-registry`.
- Workflow / Worker Plane: explicit workflow definitions, queue claiming, ingest/retrieval/memory/report/static-page/media workers.
- External Execution Host: `codex-host-agent`, isolated host workspace, Codex execution safety gates, redacted logs, and returned artifacts.
- Data / Artifact Plane: PostgreSQL, object storage, vector indexes, analytical files, and published artifacts.

Current architecture decisions:

- `llm-gateway` is the model gateway seed. A future `model-proxy` should wrap or extract it, not duplicate provider routing.
- `codex-host-agent` is an external execution worker. It must not become a browser API, model gateway, permission authority, or memory source of truth.
- Existing workers remain V3 worker-plane components. Do not move routine parsing, retrieval, static-page rendering, report rendering, or memory refresh into Codex Host.
- Codex Host tasks should reuse `workflow-definitions`, `workflow-engine`, `event-bus`, PostgreSQL task/artifact state, and runtime inspect.
- The current crate dependency exceptions are known: `codex-host-agent -> platform-api`, several workers -> `platform-api`, and `tool-registry -> llm-gateway`. Treat these as short-term seams to document and gradually clean, not as a reason to block product work immediately.

Use `docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md` before continuing Codex Host or model-proxy implementation. That plan is the active boundary checklist and dependency cleanup roadmap.

## Architecture Evaluation

The static-page plan and OpenClaw plan do not conflict if the boundary is kept strict.

Compatible points:

- Static-page provider-backed intent already goes through `llm-gateway::LlmProvider`; OpenClaw can slot in as another provider.
- AssistantRun ordinary chat already builds V3 evidence state before calling a provider; OpenClaw can receive the same already-filtered context.
- Hidden conversation memory exists in V3; OpenClaw memory can be an additional labeled evidence candidate.
- Continuous execution exists as AssistantRun continuation; future Codex Host execution should be a bounded V3-owned capability bridge recorded in the same trail.

Potential conflicts and decisions:

- OpenClaw memory vs V3 memory: V3 remains source of truth. OpenClaw memory is only optional evidence.
- External local execution vs V3 host actions: V3 validates, records, and owns actions. Codex Host may run allowlisted tasks only through V3-issued task contexts. OpenClaw readonly bridge remains optional and lower priority.
- OpenClaw model routing vs model config: OpenClaw can route model/agent choices, but runtime manifests must still record provider/model/request id in V3.
- OpenClaw availability vs production reliability: all OpenClaw lanes must be config-gated and fallback-safe.

## Host-Controlled ReAct Decision

V3 should adopt ReAct as a controlled AssistantRun execution pattern, not as a free-form autonomous agent.

Definition:

```text
Observation -> Model proposes next_action -> V3 validates action -> V3 executes host tool -> Observation persisted -> Model continues or final_answer
```

Allowed first-version action types:

- `retrieve_evidence`
- `read_document_detail`
- `recall_conversation_memory`
- `create_static_page_draft`
- `update_static_page_module`
- `submit_static_page_image_preview`
- `render_static_page`
- `create_report_draft`
- `openclaw_memory_recall`
- `openclaw_readonly_execution`
- `final_answer`

Rules:

- The model may propose actions, but V3 validates and executes them.
- V3 must apply dataset visibility, local-key matching, tool allowlists, operation schemas, and confirmation gates before any action runs.
- Planning catalog, startup briefing, visible library lists, and system capability summaries are not evidence. They only help the model choose the next tool.
- When selected datasets or conversation memory are in scope, `final_answer` requires a prior supply observation or already supplied evidence state. Premature terminal answers should receive a `policy_observation` and continue the loop.
- Protocol repair is different from fallback: repairable mistakes stay inside the same ReAct loop; fallback is reserved for runtime outage, malformed JSON that cannot be parsed safely, infrastructure failure, or step-limit exhaustion.
- ReAct observations are persisted as AssistantRun events and concise execution-trail steps.
- The UI shows brief user-facing steps, not full hidden chain-of-thought.
- Each run has a bounded step limit, default 3 and max 5 for the first implementation.
- If the model emits invalid action JSON, V3 records the failure and asks for a corrected action or uses clearly labeled runtime fallback only when safe.
- OpenClaw can participate as a provider or optional capability bridge, but ReAct state remains V3-owned.

Detailed refinement handoff: use `docs/plans/2026-04-29-v3-react-agent-refinement-plan.md` when continuing AssistantRun/ReAct work. That document is the stricter implementation checklist for moving from the first V3 product-action loop toward the Java-reference-aligned model-tool protocol.

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
- Host-Controlled ReAct lets the model request retrieval, draft updates, preview generation, report actions, or final answers through V3-validated action steps instead of one-shot prompting.

### Workstream C: Codex Host Kernel And Separated Memory

Priority: high and foundational.

Goal: make V3 safe for Codex-as-execution-kernel by separating memory and routing host tasks through V3-owned ReAct/tool policy.

Next outcomes:

- `conversation`, `project`, `task`, `dataset`, and `system` memory spaces exist as first-class V3 concepts.
- AssistantRun stores the active memory space and never recalls unrelated thread/project memory implicitly.
- ReAct can select, recall, and write memory only through V3 validation.
- Codex Host task actions are disabled by default, allowlisted, audited, and routed through a dedicated queue before any real host execution.
- Runtime inspect shows memory-space decisions, recalled counts, denied counts, and Codex task context summaries.

Current Codex Host status: the V3 queue bridge, dry-run worker, and plan-only command policy are implemented; first-class task memory spaces, shared Codex Host contracts, dependency decoupling from `platform-api`, and real jump-host/Mac-host Codex execution are still pending.

Detailed plans: `docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md`, `docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md`, and `docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md`.

### Workstream C2: Optional OpenClaw Extension

Priority: completed first pass; continue only for concrete compatibility bugs or provider routing needs.

Goal: keep OpenClaw as optional sidecar for model configuration and legacy compatibility without making it the main execution kernel.

Next outcomes:

- Already completed: `llm-gateway` has `OpenClawProvider`.
- Already completed: AssistantRun and static-page intent can use `*_RUNTIME_PROVIDER=openclaw`.
- Already completed: gated ReAct stubs exist for OpenClaw memory/readonly bridge.
- Do not prioritize `crates/openclaw-extension` unless OpenClaw becomes specifically required again.

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

Goal: keep email account auth, local-key access factors, provider secrets, and background services safe.

Next outcomes:

- Add account email capture in the existing key panel.
- Add email + key login and email verification-code login.
- Add recovery flow that restores account access and lets the user set a new local key, without claiming to recover the old raw key.
- Use Cloudflare Email Service as the verification-code delivery channel through a dedicated sender address.
- Attach datasets, robots, memory, conversations, reports, static pages, and artifacts to user ownership.
- Local key UX and backend visibility enforcement are consistent.
- Provider keys stay server-side or local-only as intended and never leak through UI/status.
- PostgreSQL 17.9 remains the target server version for fresh environments.
- Runtime status endpoints are redacted and useful.

Detailed plan: `docs/plans/2026-05-07-v3-email-account-auth-plan.md`.

## Recommended Execution Order

### Slice 0: Commit Consolidated Plans

**Files:**

- Add: `docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md`
- Add: `docs/plans/2026-04-29-openclaw-extension-adapter-plan.md`
- Add: `docs/plans/2026-04-29-v3-react-agent-refinement-plan.md`
- Modify: `docs/plans/2026-04-27-static-page-generation-studio-plan.md`

**Steps:**

1. Add consolidation notes to old source plans.
2. Add the ReAct refinement sub-plan if AssistantRun/ReAct work is active.
3. Run `git diff --check`.
4. Commit as `docs: consolidate v3 development handoff plan`.

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

### Slice 3: Host-Controlled ReAct AssistantRun Loop

**Status 2026-04-29:** first ReAct refinement pass completed.
AssistantRun create and continue paths now support a config-gated Host-Controlled ReAct loop with strict typed action JSON parsing, weak planning catalog, step limits, V3-owned retrieval/visibility checks, bounded `read_document_detail`, report handoff invariants, static-page module operation sanitization, redacted ReAct trace summaries, safe frontend progress display, and gated OpenClaw bridge stubs. Contracts/storage migrations were not required for this pass because existing AssistantRun events, output artifacts, runtime manifest, and execution-trail fields were sufficient. Static-page write actions are still returned as sanitized operations/observations rather than silently mutating arbitrary drafts; applying them to the selected current draft belongs to Slice 4.

**Reference alignment 2026-04-29:** compared with `C:/Users/soulzyn/Desktop/codex/ai-data-platform-java-client/docs/architecture/react-agent-architecture-reference.md`.
The Java reference confirms the same boundary: model owns intent/next action/final wording, while V3 owns identity, scope, permission checks, whitelisted tool execution, limits, and audit. The implemented V3 pass now keeps planning catalogs weak, treats observations/evidence state as answerable supply, repairs premature terminal answers and report choices with `policy_observation`, returns denied IDs only, avoids raw observations in persisted ReAct events, and reserves fallback for runtime degradation rather than normal protocol steering.

**Detailed refinement plan 2026-04-29:** `docs/plans/2026-04-29-v3-react-agent-refinement-plan.md` records the completed first refinement pass and remains the reference for future hardening.

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/domain-model/src/lib.rs` only if a typed domain enum is needed
- Modify: `crates/storage/src/lib.rs` only if existing AssistantRun events/output artifacts are insufficient
- Modify: `crates/static-page-runtime/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Define a strict `AssistantRunNextAction` JSON contract with `action_type`, `reason_summary`, `arguments`, and `requires_confirmation`.
2. Add a parser/sanitizer that rejects unknown action types, unsafe keys, excessive argument size, and write actions outside the allowlist.
3. Add host action router for first-version actions: `retrieve_evidence`, `recall_conversation_memory`, `update_static_page_module`, and `final_answer`.
4. Persist each step as AssistantRun event names such as `assistant_run.react.action_requested`, `assistant_run.react.action_completed`, and `assistant_run.react.final_answer`.
5. Add concise execution-trail labels for the UI, for example `检索供料证据`, `召回对话记忆`, `更新静态页模块`, `模型生成最终回答`.
6. Enforce max-step limits with default 3 and max 5.
7. Add protocol repair observations for premature terminal answers, report-choice-before-options, denied IDs, and repairable policy violations.
8. Keep deterministic fallback only for runtime outage, malformed JSON that cannot be parsed safely, infrastructure failure, explicit ReAct disablement, or step-limit exhaustion.
9. Add tests for valid retrieval action, invalid action rejection, hidden dataset denial, static-page module update action, final answer, premature final-answer repair, and step-limit stop.
10. For post-first-slice cleanup, follow the task order in `docs/plans/2026-04-29-v3-react-agent-refinement-plan.md` instead of expanding the monolithic `crates/platform-api/src/lib.rs` implementation further.
11. Completed refinement pass added `read_document_detail`, `list_report_options`, `report_choice`, redacted `react_trace`, safe frontend progress, and gated `openclaw_memory_recall` / `openclaw_readonly_execution` stubs.

**Acceptance:**

- ReAct never bypasses V3 visibility or tool allowlists.
- AssistantRun can continue through multiple model-requested action steps.
- UI-facing trail contains concise steps without exposing hidden chain-of-thought.
- Static-page module updates can be requested by the model through the same action contract used by user micro-adjustments.
- Premature final answers over scoped private/public data are repaired inside the ReAct loop instead of accepted as business answers.
- Existing ordinary chat still works when ReAct is disabled.

**Verified 2026-04-29:**

- `cargo check -p platform-api`
- `cargo test -p platform-api assistant_run_react`
- `cargo test -p platform-api react_agent_contract`
- `cargo test -p platform-api react_agent_tools`
- `cargo test -p platform-api openclaw_react`
- `cargo test -p platform-api assistant_run`
- `npm run build` in `apps/web`

### Slice 4: Static-Page Module Editing And Data Binding

**Status 2026-04-29:** first contract/UI batch implemented.
The frontend draft model now exposes data-source candidates, stronger module update operations, normalized `dataBinding`, `visualization`, and `chartOptions`, and image/final render payloads carry the same editable contract. The module micro-adjustment UI emits the stronger `update_module` operation while keeping controls tucked inside the existing collapsed editor. `static-page-runtime` now validates the stronger operation schema and provider prompts explicitly tell models to use it. `platform-api` propagates data-source candidates and chart options through draft payload/data snapshots. Remaining Slice 4 work should focus on richer real field discovery from selected evidence and broader UI affordances only where they stay natural-language-first.

**Status 2026-04-29 field discovery batch:** in progress.
Static-page data snapshots now derive `field_candidates` from selected scope, AssistantRun evidence state, retrieval summaries/excerpts, and retrieval term weights. Frontend draft snapshots preserve backend field suggestions, add safe evidence fallbacks, and expose those fields through a lightweight datalist inside the collapsed module editor. This keeps module data binding model-operable without turning the product into a manual form builder.

**Status 2026-04-29 ReAct draft mutation batch:** completed locally.
`update_static_page_module` can now apply sanitized module operations directly to the current persisted static-page draft when the active AssistantRun matches the draft owner. The operation path updates the draft payload, status, metadata, and AssistantRun audit event, while create-path or non-current drafts still return sanitized operations without mutating storage. The web client refreshes the active backend draft after AssistantRun responses so natural-language module edits show in the main workspace quickly.

**Status 2026-04-29 AssistantRun routing batch:** completed locally.
Static-page creation and backend-synced static-page edits now prefer AssistantRun even when a dataset is selected, so selected scope is supplied to the model and module edits can use the ReAct tool path. The local prompt interpreter remains only as a fallback when there is no backend draft/run or AssistantRun is unavailable.

**Status 2026-04-29 ReAct preview/render action batch:** completed locally.
`submit_static_page_image_preview` and `render_static_page` now execute real V3 backend actions for the current persisted static-page draft only. The Host verifies the draft belongs to the active AssistantRun before queueing image preview generation or creating the final static-page render. Rendering before preview confirmation is rejected as a structured ReAct observation instead of aborting the loop.

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

**Verified 2026-04-29 first batch:**

- `node --test apps/web/app/lib/static-page-draft.test.mjs`
- `cargo test -p static-page-runtime`
- `cargo test -p platform-api static_page_draft_can_be_created_under_assistant_run`
- `npm run build` in `apps/web`

**Verification target for field discovery batch:**

- `node --test apps/web/app/lib/static-page-draft.test.mjs`
- `cargo test -p platform-api static_page_data_snapshot_extracts_field_candidates_from_evidence_state`
- `cargo test -p platform-api static_page_draft_can_be_created_under_assistant_run`
- `cargo test -p static-page-runtime`
- `npm run build` in `apps/web`

**Verification note 2026-04-29 ReAct draft mutation batch:**

- Rust checks passed before the final create-path guard tightening: `cargo test -p platform-api assistant_run_react_static_page_module_update_applies_current_backend_draft`, `cargo test -p platform-api assistant_run_react_provider_input_uses_weak_planning_catalog`, `cargo test -p platform-api assistant_run_react`, and `cargo test -p static-page-runtime`.
- The current shell no longer exposes `cargo`/`rustc`; rerun the Rust checks when the Rust toolchain PATH is restored.
- `node --test apps/web/app/lib/static-page-draft.test.mjs apps/web/app/lib/scope-planner.test.mjs`
- `npm run build` in `apps/web`

**Verified 2026-04-29 ReAct preview/render action batch:**

- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react_static_page"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-renderer"`

### Slice 5: Real Static-Page Data Snapshot Path

**Status 2026-04-29 initial renderer batch:** in progress.
The backend data snapshot now attaches per-module evidence-derived `sampleData` and `dataQuality` when a module has a recognized `fieldPath`. The renderer now reads module `dataBinding`, snapshot module bindings, and snapshot `sampleData` before drawing charts. If chart data is missing, it renders an explicit "数据待确认" notice instead of fake placeholder numbers.

**Status 2026-04-29 explicit evidence values batch:** completed locally.
Static-page data snapshots now prefer explicit numeric values extracted from retrieval evidence text before falling back to keyword signal scores. Evidence lines such as `1月,订单金额,1200` become chart-ready `sampleData` with `kind: "evidence_value"` and `dataQuality: "evidence_value"`. The renderer has a regression test proving final HTML uses snapshot sample values instead of showing missing-data placeholders.

**Status 2026-04-29 explicit module data contract batch:** completed locally.
Static-page data snapshots now also prefer user/model-edited module data before falling back to retrieved evidence or keyword signal scores. This supports natural-language module data edits such as `visualization.data: [{ month, amount }]` and records them as `sampleData` with `kind: "module_data"` / `dataQuality: "module_data"`. The platform has a regression test proving the same `dataSnapshot` is sent to the image prompt payload and final render asset manifest, while the renderer now accepts common label/value aliases such as `month`, `x`, `amount`, `y`, `金额`, and numeric strings.

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

**Verification target for initial renderer batch:**

- `cargo test -p platform-api static_page_data_snapshot_extracts_field_candidates_from_evidence_state`
- `cargo test -p static-page-renderer`
- `cargo test -p platform-api static_page_draft_can_be_created_under_assistant_run`
- `npm run build` in `apps/web`

**Verified 2026-04-29 explicit evidence values batch:**

- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all --check"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page_data_snapshot"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-renderer"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-runtime"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page_draft_can_be_created_under_assistant_run"`
- `npm run build` in `apps/web`

**Verified 2026-04-29 explicit module data contract batch:**

- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all --check"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page_data_snapshot"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-renderer"`

### Slice 6: Final Render Worker And Export Package

**Status 2026-04-29 background render workflow foundation:** completed locally.
The domain/workflow contracts now include `static_page_render_workflow` and render output lifecycle states `queued`, `rendering`, `rendered`, `failed`, and `cancelled`. The existing render endpoint remains backward-compatible by default, while `background: true` queues a `render_static_page` workflow task and returns a queued render output. `static-page-worker` can now claim both static-page image and render tasks, render confirmed drafts into durable HTML/manifests, update the draft final page payload, and mark render outputs failed if the worker path errors. Generic workflow cancel/retry/start signals also sync the render output workflow manifest so the web UI can later reflect queued, failed, or cancelled state from the artifact itself.

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

**Verified 2026-04-29 background render workflow foundation:**

- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all --check"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p static-page-worker"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p workflow-definitions"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page_draft_can_be_created_under_assistant_run"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react_static_page"`
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-worker"`

**Remaining Slice 6 work:**

- Connect the web UI to background render mode after queued/rendering UX is ready.
- Add dedicated static-page render retry/cancel controls in the web UI, using the existing workflow signal routes.
- Add worker-level integration coverage for failed render tasks and cancelled outputs.
- Package/export rendered HTML and assets into a downloadable artifact.

### Slice 7: Codex Kernel Separated Memory Foundation

**Status 2026-05-03:** supersedes the old OpenClaw memory bridge as the next architecture slice.

**Plan:** `docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md`

**Files:**

- Create: `docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md`
- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Create: `crates/storage/migrations/0003_memory_spaces.sql`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/react_agent_contract.rs`
- Modify: `crates/platform-api/src/react_agent_catalog.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `apps/web/app/HomePageClient.js`
- Create: `apps/web/app/lib/memory-space.js`

**Steps:**

1. Add `memory_spaces` and attach conversation memory to memory spaces.
2. Ensure each browser `local_thread_id` gets an isolated conversation memory space.
3. Attach AssistantRun to an active memory space and return memory candidates without raw content.
4. Add ReAct memory actions for selecting, recalling, writing, and archiving memory through V3 validation.
5. Add minimal frontend memory status and new-conversation handling.
6. Add Codex Host disabled-by-default task action and isolated task memory semantics.
7. Extend runtime inspect with memory-space decisions and denied cross-scope access.

**Acceptance:**

- Conversation memory cannot leak between browser threads.
- Project memory is explicit, not silently inferred.
- Codex task memory is isolated from normal conversation memory.
- Dataset/private visibility remains enforced before memory is supplied.
- OpenClaw remains optional and does not block this slice.

### Slice 7 Legacy: OpenClaw Memory Bridge

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

**Status 2026-05-03:** deprioritized. Keep this only as a future optional sidecar task after Codex Host separated memory is in place.

### Slice 8 Legacy: OpenClaw Readonly Local Execution Bridge

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

**Status 2026-05-03:** deprioritized. The first external execution bridge should target Codex Host disabled-by-default task actions, not OpenClaw local execution.

### Slice 9: Parser And Media Parity

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

### Slice 10: Report And Artifact Shelf Integration

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

### Slice 11: Security And Operations Hardening

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `apps/web/app/HomePageClient.js`
- Create or modify runtime docs

**Steps:**

1. Implement email account auth from `docs/plans/2026-05-07-v3-email-account-auth-plan.md`.
2. Add redacted OpenClaw/provider status endpoint where still relevant.
3. Add provider/runtime/email-auth status to operator docs.
4. Improve local key create/switch/revoke/recovery UX.
5. Ensure private/public dataset warnings are clear during upload and dataset creation.
6. Verify PostgreSQL 17.9 fresh setup path against current migrations.

**Acceptance:**

- Email account and local-key model remains simple and understandable.
- Users can log in with email + key or email verification code.
- Forgot-key recovery restores account access and new-key setup, but never exposes the old raw key.
- Wrong-key private datasets are invisible and unusable.
- Provider secrets are never shown to the browser.

## Immediate Next Thread Instruction

Current 2026-05-07 transition note: OpenClaw provider/stub work has completed its optional first pass, and the main execution-kernel direction is now Codex Mac Host plus separated memory. The Codex Host ReAct action, workflow queue, and dry-run worker exist. A fresh thread continuing Codex Host should first read `docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md`, then continue the shared-contract and dependency-decoupling tasks before real execution. Real Codex execution validation must happen only on `windows-jump` or the later Mac host.

Recommended next prompt:

```text
Continue AI Data Platform V3 from docs/plans/2026-05-07-v3-codex-host-architecture-alignment-plan.md.
OpenClaw provider/stubs already completed their optional first pass; do not continue OpenClaw as the main execution kernel.
Codex Host queue/dry-run/plan-only bridge is implemented; continue by linking the architecture plan, auditing crate boundaries, moving Codex Host payloads into shared contracts, and reducing codex-host-agent -> platform-api coupling before real execution.
Keep V3 as the control plane, llm-gateway as the model-provider seed, and Codex Host as an external execution worker. Browser/API traffic must still go only through V3.
Do not run local-machine Codex from the developer workstation.
Use separated memory rules: conversation/project/task/dataset/system memory must not leak across scopes.
```

Older 2026-04-29 continuation guidance remains below for historical context and for threads that are specifically continuing static-page implementation.

Start with Slice 0 if the consolidated plan is not committed.

Current 2026-04-29 transition note: the first ReAct refinement pass is complete. A fresh thread should not restart contract/catalog/tool-registry work unless tests fail or requirements change. Continue with Slice 4 static-page module mutation/data-binding integration, because that is the core customer-facing product loop.

If Slice 0 is already committed but Slice 1 and Slice 2 are not yet committed, start with Slice 1:

```text
Implement optional OpenClawProvider in crates/llm-gateway.
Use docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md as the active plan.
Do not change static-page behavior while adding the provider.
Keep OpenClaw disabled by default and fully fallback-safe.
```

If Slice 1 and Slice 2 are already committed but Slice 3 is not, start with Slice 3:

```text
Implement Host-Controlled ReAct inside AssistantRun.
Use docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md as the active plan.
The model may propose actions, but V3 validates, executes, records, and limits every action.
Keep ordinary chat and deterministic static-page fallback working when ReAct is disabled.
```

If Slice 3 first backend slice is committed but the detailed ReAct refinement is not complete, start with the ReAct refinement sub-plan:

```text
Refine Host-Controlled ReAct using docs/plans/2026-04-29-v3-react-agent-refinement-plan.md.
Begin at the first incomplete task: typed contract module, weak planning catalog, tool registry, protocol repair, report handoff, document detail, trace, frontend progress, or OpenClaw bridge.
Keep current V3 product actions working while extracting structure from crates/platform-api/src/lib.rs.
Do not add OpenClaw bridge actions until the registry and protocol repair matrix are in place.
```

If the first ReAct refinement pass is complete, continue Slice 4:

```text
Implement static-page module editing and data binding.
Use docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md as the active plan.
Let model-requested update_static_page_module operations target the current draft only through V3 schemas.
Keep draft mutation behind visibility checks, confirmation/staleness rules, and sanitized operation contracts.
Add tests for title/content edits, visualization switch, chart/data binding patches, and stale preview/final render markers.
```

Reason:

- It is the visible product value: users can talk naturally, then refine module titles, text, data binding, chart type, and layout.
- ReAct now provides the safe execution frame for model-requested module edits.
- The right shelf/main workspace model is already in place; the next gains should be draft mutation reliability and visual/data fidelity.
- OpenClaw remains optional and should not distract from static-page core unless a concrete bridge adapter is being tested.

ReAct is now the execution frame that lets natural-language requests safely drive module edits, retrieval, preview generation, and report/static-page actions. Continue Slice 4 before deeper OpenClaw/local-execution work unless the user explicitly asks to prioritize the sidecar.

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
cargo test -p platform-api assistant_run_react
cargo test -p platform-api react_agent_contract
cargo test -p platform-api react_agent_tools
cargo test -p platform-api openclaw_react
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
