# AI Data Platform V3 Development Plan

Date: 2026-04-23
Status: Working plan

## Positioning

V3 is past the compile-oriented skeleton stage. The platform now has a durable
workflow/task runtime, worker slices, report host surfaces, runtime inspect,
tool registry entries, and a first web assistant shell.

The next goal is not to add more disconnected skeletons. The goal is to turn the
existing host surfaces into product loops, then tighten the runtime semantics
that make those loops trustworthy.

## Guiding Rules

- Keep PostgreSQL as the system of record.
- Keep NATS as an acceleration layer, never the only source of truth.
- Keep chat as weak orchestration by default; Report Service requires an
  explicit host-side gate.
- Prefer thin host surfaces and typed views over prompt-only state.
- Verify every slice with Rust checks and web build before pushing.

## Phase 0: Repository Hygiene And Baseline

Goal: make `main` a trustworthy baseline after the first GitHub push.

Tasks:

- Decide and lock dependency drift.
- Keep the verified Next 16 route handler adaptation if the build remains green.
- Keep the verified Rust dependency upgrades only after `cargo check` and tests.
- Update root README from skeleton-era wording to current state.
- Keep `apps/web/README.md` aligned with actual dev ports and proxy behavior.

Validation:

- `pnpm --filter @ai-data-platform-v3/web build`
- `wsl bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check --workspace'`
- `wsl bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test'`

## Phase 1: Web Report Service Loop

Goal: turn `report.plan`, `report.render`, `report.publish`, and
`report.read_published` into a usable front-end loop.

Tasks:

- Add report plan detail in the web app.
- Add continue-planning action for draft or stale plans.
- Add render action with `pc` / `mobile` surface selection.
- List render outputs and their runtime status.
- Add publish action after successful render.
- Add published report detail view, including current version and versions.
- Surface failures from host APIs instead of optimistic success messaging.

Validation:

- A user can enter Report Service from `chat_session.report_entry`.
- A report plan can be rendered from the web UI.
- A rendered report can be published from the web UI.
- Published report detail can be read from the web UI.

Progress:

- 2026-04-23: Added the first web Report Service control panel. It can select a
  report plan, load AST versions, render outputs, and published detail, and call
  the existing continue/render/publish host actions.

## Phase 2: Existing-Session Chat Turn Append

Goal: make the assistant a continuous conversation surface instead of a
session-creation launcher.

Tasks:

- Add a formal append-turn API, likely `POST /v1/chat-sessions/{session_id}/turns`.
- Persist appended user messages with stable `turn_index` sequencing.
- Create a new workflow execution for each appended turn.
- Ensure worker context uses existing session history and latest dataset state.
- Update `ChatSessionView.latest_assistant_message` after each appended turn.
- Update web composer to append to selected session by default.
- Keep "new conversation" as an explicit user action.

Validation:

- Three consecutive turns can be appended to one session.
- Message ordering remains stable.
- Retry does not duplicate assistant messages.
- Report-entry gate remains functional in multi-turn sessions.

## Phase 3: Mobile-Usable Web Shell

Goal: make the current responsive UI truly usable on mobile web.

Tasks:

- Convert sidebar dataset selection into a mobile drawer or top selector.
- Convert the right insight panel into tabs or a bottom sheet.
- Make the composer keyboard-safe.
- Improve touch targets and long-text wrapping.
- Ensure report-entry confirmation is usable on narrow screens.

Validation:

- No horizontal scrolling on common mobile widths.
- Dataset switching, session selection, message sending, and report entry are
  usable in mobile viewport.

## Phase 4: Runtime Semantics

Goal: move from typed shell to trustworthy runtime facts.

Tasks:

- Tighten provider lifecycle states.
- Stabilize streaming fields and state transitions.
- Stabilize tool-loop requested/completed/failed semantics.
- Continue closing artifact commit recovery gaps.
- Extend runtime inspect summaries where they help UI and operations.

Validation:

- Provider success/failure/timeout are durable facts.
- Tool loop states can be reconstructed from durable rows.
- Streaming timestamps and artifact commit timestamps do not contradict each
  other.
- `runtime.inspect` remains consistent across API and CLI.

## Phase 5: Real Retrieval

Goal: make evidence state derive from real retrieval rather than placeholders.

Tasks:

- Add embedding production.
- Add vector index integration.
- Store recall scores and rank hints from real retrieval.
- Make document detail and compare consume real evidence.
- Tighten `catalog_memory`, `supply_only`, `live_detail`, `mixed`, and
  `degraded` derivation.

Validation:

- Ingested document chunks can be recalled by query.
- Single-document detail can reach `live_detail`.
- Multi-document comparison can reach `mixed`.
- Retrieval failures produce `degraded` instead of unsupported confidence.

## Phase 6: Model-Facing Contract Freeze

Goal: make the model-facing protocol stable after runtime facts are strong
enough.

Tasks:

- Freeze `capability_class` meanings.
- Freeze `service_lane` meanings.
- Freeze `evidence_state` meanings.
- Freeze `continuation_state` meanings.
- Freeze `recommended_tool_key` and `allowed_tool_keys` mapping.
- Align runtime inspect, tool registry, and UI around the same contract.

Validation:

- The UI no longer needs to infer platform next actions manually.
- Material Service remains default.
- Report Service remains gated by explicit confirmation.
- Tool registry, inspect, and typed views agree on recommended host actions.

## Phase 7: CI And Operations

Goal: make the project reproducible beyond the local desktop.

Tasks:

- Add GitHub Actions for Rust checks and web build.
- Document local compose startup.
- Document required environment variables.
- Add worker startup recipes.
- Add migration and release checklist.

Validation:

- Fresh checkout can follow README and run API + workers + web.
- CI catches Rust or web regressions before merge.
