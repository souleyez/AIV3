# V3 ReAct Agent Refinement Implementation Plan

> **For Codex:** REQUIRED SKILL: Use `executing-plans` to implement this plan task-by-task.

**Goal:** Refine V3 AssistantRun ReAct from the current product-action loop into a Java-reference-aligned model-tool protocol while preserving V3 static-page/report/product actions.

**Architecture:** Keep V3 as the enforcement boundary and keep the model as owner of intent, next action, and final wording. Port the Java client's stronger ReAct discipline into Rust: weak planning catalog, typed decision contract, protocol repair, tool registry, trace redaction, and report handoff invariant. Do not replace the current AssistantRun and static-page integration; gradually extract the current `platform-api` ReAct `match` into reusable tools.

**Tech Stack:** Rust `platform-api`, `contracts`, `domain-model`, `storage`, `static-page-runtime`, `static-page-renderer`; PostgreSQL 17.9 target; Next.js 16 frontend; optional OpenClaw provider via `llm-gateway`.

---

## 0. Current State And Source References

Current V3 state:

- `crates/platform-api/src/lib.rs` already has config-gated AssistantRun ReAct create/continue loops.
- V3 already validates selected dataset scope for retrieval.
- V3 already sanitizes static-page module operations through `static-page-runtime`.
- V3 already persists ReAct events as `AssistantRunEvent` and concise execution trail steps.
- V3 now repairs premature `final_answer` for scoped dataset/conversation-memory runs with a `policy_observation`.

Reference implementations:

- Java architecture reference: `C:/Users/soulzyn/Desktop/codex/ai-data-platform-java-client/docs/architecture/react-agent-architecture-reference.md`
- Java contract: `C:/Users/soulzyn/Desktop/codex/ai-data-platform-java-client/docs/api-contracts/chat-react-agent-contract.md`
- Java runner: `C:/Users/soulzyn/Desktop/codex/ai-data-platform-java-client/backend/api-service/src/main/java/com/aidataplatform/api/chat/react/ReActChatRunner.java`
- Java tool registry: `C:/Users/soulzyn/Desktop/codex/ai-data-platform-java-client/backend/api-service/src/main/java/com/aidataplatform/api/chat/react/ReActToolRegistry.java`
- Java prompt builder: `C:/Users/soulzyn/Desktop/codex/ai-data-platform-java-client/backend/api-service/src/main/java/com/aidataplatform/api/chat/ModelLedChatPromptBuilder.java`
- Original TS parser contract: `C:/Users/soulzyn/Desktop/codex/ai-data-platform/apps/api/src/lib/react-agent-contract.ts`

Design decision:

- Java client has the better generic ReAct architecture.
- V3 has the better product-action integration.
- Implement the hybrid: Java discipline + V3 product tools.

## 1. Target Contract

V3 should converge on this model-facing shape:

```json
{
  "status": "act|final_answer|report_choice|ask_clarifying_question|cannot_answer",
  "intent": "question|report|static_page|other",
  "reason": "short operational summary, not hidden reasoning",
  "action": {
    "type": "retrieve_evidence|read_document_detail|recall_conversation_memory|list_report_options|create_report_draft|create_static_page_draft|update_static_page_module|submit_static_page_image_preview|render_static_page|openclaw_memory_recall|openclaw_readonly_execution",
    "arguments": {}
  },
  "answer": "final answer or short clarification",
  "citations": [],
  "conversationState": {}
}
```

Compatibility rule:

- During migration, still accept current V3 `action_type/reason_summary/arguments/requires_confirmation`.
- Normalize both shapes into one internal `AssistantRunReActDecision`.
- After migration, prefer `status=act` with `action.type` for all non-terminal actions.

Terminal rules:

- `final_answer` over selected datasets or conversation memory requires prior supply observation or already supplied evidence.
- `report_choice` requires prior `list_report_options` observation.
- `ask_clarifying_question` and `cannot_answer` can terminate without supply.
- Planning catalog is never evidence.

## 2. Task Breakdown

### Task 1: Add A Typed ReAct Contract Module

**Files:**

- Create: `crates/platform-api/src/react_agent_contract.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_contract.rs`

**Step 1: Write parser tests first**

Add tests for:

- Current V3 shape parses:

```json
{"action_type":"retrieve_evidence","reason_summary":"检索资料","arguments":{"query":"订单风险"},"requires_confirmation":false}
```

- Java-reference shape parses:

```json
{"status":"act","intent":"question","reason":"检索资料","action":{"type":"retrieve_evidence","arguments":{"query":"订单风险"}}}
```

- Terminal `final_answer` parses from both `arguments.content` and top-level `answer`.
- Unknown status is rejected.
- Unknown action is rejected.
- Unsafe keys `__proto__`, `constructor`, `prototype` are rejected.
- Oversized reason is truncated to 240 chars.

**Step 2: Implement internal structs**

Use these internal concepts:

- `AssistantRunReActStatus`
- `AssistantRunReActActionType`
- `AssistantRunReActDecision`
- `AssistantRunReActCitation`
- `AssistantRunReActParseError`

Keep serialization local to `platform-api` for now. Do not touch public `contracts` unless frontend/API needs these types later.

**Step 3: Wire old parser through new module**

Replace `parse_assistant_run_next_action` in `crates/platform-api/src/lib.rs` with a call into the new module.

**Step 4: Verify**

Run:

```powershell
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api react_agent_contract'
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react'
```

Expected:

- New contract tests pass.
- Existing AssistantRun ReAct tests still pass.

**Commit:** `refactor(platform-api): extract assistant react contract`

### Task 2: Make The Planning Catalog Explicitly Weak

**Files:**

- Create: `crates/platform-api/src/react_agent_catalog.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_catalog.rs`

**Step 1: Write catalog tests**

Tests should verify planning catalog includes:

- Dataset IDs, titles, visibility, document counts if available.
- Document IDs, titles, parse/vector/profile status, updated time if available.
- System capabilities such as `static_page`, `report`, `retrieval`, `conversation_memory`.

Tests should verify planning catalog excludes:

- Document body text.
- Chunk text.
- OCR text.
- Table text.
- Long summaries.
- Raw profile values.
- Provider keys or local access keys.

**Step 2: Implement catalog builder**

Create a helper similar to Java `reActPlanningCatalog`:

```rust
fn build_assistant_run_react_planning_catalog(
    startup_briefing: &Value,
    scope_candidates: &[Value],
    selected_scope: &Value,
    evidence_state: &Value,
) -> Value
```

Keep it conservative. It should help the model choose tools, not answer.

**Step 3: Use catalog in create/continue prompts**

Replace full `startup_briefing` and broad visible context in ReAct prompts with:

- `planning_catalog`
- `selected_scope`
- `observations`
- `current_artifact`
- bounded recent chat history

**Step 4: Verify**

Run:

```powershell
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api react_agent_catalog'
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react'
```

Expected:

- Prompt tests prove private body/chunk text is absent from planning catalog.
- Existing ReAct behavior still works.

**Commit:** `feat(platform-api): add weak react planning catalog`

### Task 3: Extract A ReAct Tool Registry

**Files:**

- Create: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_tools.rs`

**Step 1: Write tool registry tests**

Tests should cover:

- Unknown action returns denied observation, not panic.
- Tool registry only executes registered tools.
- Denied IDs are returned as identifiers only.
- Tool result includes `actionType`, `status`, `message`, `denied`, `items`, `limits`.

**Step 2: Define registry shape**

Use a simple Rust enum/dispatcher first, not trait objects unless necessary:

- `RetrieveEvidenceTool`
- `ReadDocumentDetailTool`
- `RecallConversationMemoryTool`
- `ListReportOptionsTool`
- `CreateStaticPageDraftTool`
- `UpdateStaticPageModuleTool`
- `SubmitStaticPageImagePreviewTool`
- `RenderStaticPageTool`
- `OpenClawMemoryRecallTool`
- `OpenClawReadonlyExecutionTool`

The first extraction may implement only existing actions:

- `retrieve_evidence`
- `recall_conversation_memory`
- `update_static_page_module`
- `final_answer` remains terminal, not a tool

Stub unimplemented tools with safe `rejected` observations.

**Step 3: Move current `match` branches**

Move existing logic from `execute_assistant_run_react_action` into tool functions. Keep behavior unchanged.

**Step 4: Verify**

Run:

```powershell
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api react_agent_tools'
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react'
```

Expected:

- Existing tests pass.
- New registry tests prove unknown/denied tools are safe.

**Commit:** `refactor(platform-api): extract assistant react tool registry`

### Task 4: Add Full Protocol Repair Matrix

**Files:**

- Modify: `crates/platform-api/src/react_agent_contract.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_contract.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Add repair tests**

Add tests for:

- `final_answer` before supply observation -> `policy_observation`.
- `report_choice` before `list_report_options` -> `policy_observation`.
- Denied dataset/document action while passes remain -> denied observation and loop continues.
- Repeating same no-progress action twice -> safe fallback or `cannot_answer` prompt observation.

**Step 2: Implement repair helper**

Implement:

```rust
fn build_react_protocol_repair(
    decision: &AssistantRunReActDecision,
    observations: &[Value],
    selected_scope: &Value,
    evidence_state: &Value,
) -> Option<AssistantRunReactActionResult>
```

Rules:

- It must not choose a business action.
- It only returns policy observations.
- It must never leak private content.

**Step 3: Use helper before tool execution and terminal acceptance**

The loop order should be:

1. Parse decision.
2. Check protocol repair.
3. If repair exists, append observation and continue.
4. Else execute action or accept terminal.

**Step 4: Verify**

Run:

```powershell
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react'
```

Expected:

- All protocol repair tests pass.
- Ordinary chat scope can still `final_answer` directly.

**Commit:** `feat(platform-api): complete assistant react protocol repair`

### Task 5: Implement Report Handoff As A ReAct Tool

**Files:**

- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_tools.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write report handoff tests**

Tests:

- `report_choice` before `list_report_options` is repaired.
- `list_report_options` with selected dataset returns exactly two choices:

```json
[
  {"key":"continue_qa","label":"继续问答","type":"report_choice"},
  {"key":"create_report","label":"生成报表","type":"report_choice"}
]
```

- Requested dataset outside selected scope returns denied observation.
- Chat response does not generate report body inside `report_choice`.

**Step 2: Implement `list_report_options`**

It should:

- Use selected scope only.
- Return one permitted target.
- Return exact product-defined choices.
- Include no document body.

**Step 3: Add terminal handling for `report_choice`**

The terminal should map to a user-facing AssistantRun output artifact and/or report-entry action that frontend can render.

**Step 4: Verify**

Run:

```powershell
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react_report'
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react'
```

Expected:

- Report choice invariant is enforced.
- Existing static-page actions still work.

**Commit:** `feat(platform-api): add react report handoff tool`

### Task 6: Add Read Document Detail Tool

**Files:**

- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_tools.rs`

**Step 1: Write tests**

Tests:

- Fetching a selected-scope document returns bounded chunks.
- Fetching outside-scope document returns denied ID only.
- Returned chunks are capped by count and characters.
- OCR/table/profile fields are bounded and omitted when unavailable.

**Step 2: Implement tool**

Use existing storage/retrieval primitives if available:

- Documents table for document metadata.
- Document chunks table for chunks.
- Retrieval evidence table for evidence references.

Caps:

- Max document IDs: 3.
- Max chunks per document: 8.
- Max detail characters per tool call: 8000.

**Step 3: Add prompt examples**

Tell model:

- Use `retrieve_evidence` for candidate discovery.
- Use `read_document_detail` when exact wording/detail is needed.
- Cite from observations only.

**Step 4: Verify**

Run:

```powershell
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api react_read_document_detail'
```

Expected:

- Detail observations are bounded and scoped.

**Commit:** `feat(platform-api): add react document detail tool`

### Task 7: Persist Redacted ReAct Trace

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Add migration if this repo uses SQL migrations for local Postgres storage.
- Test: `crates/storage/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Decide storage strategy**

Preferred first step:

- Continue using `AssistantRunEvent` for full trace if it is sufficient.
- Add a redacted `react_trace` summary to `execution_trail` and runtime manifest.

Only add a new table if operations need queryable trace across runs.

**Step 2: Define redacted trace schema**

Fields:

- `trace_id`
- `assistant_run_id`
- `pass_number`
- `action_type`
- `reason_summary`
- `status`
- `denied_count`
- `returned_count`
- `duration_ms`
- `safe_error_code`
- `safe_message`

Forbidden:

- Full prompts.
- Hidden reasoning.
- Raw provider keys.
- Full document text beyond bounded previews.

**Step 3: Add tests**

Tests should assert:

- Trace exists after ReAct run.
- Trace reason is truncated.
- Observation preview is capped.
- Secrets are not present in serialized event payloads.

**Step 4: Verify**

Run:

```powershell
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react_trace'
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage react_trace'
```

Expected:

- Trace is useful for operations and safe for customer data.

**Commit:** `feat(platform-api): persist redacted assistant react trace`

### Task 8: Align Frontend Progress Display

**Files:**

- Modify: `apps/web/app/components/AssistantRunPanel.js` or current AssistantRun UI component.
- Modify: `apps/web/app/lib/assistant-run.js` if present.
- Test: existing web tests if available.

**Step 1: Locate current AssistantRun trail UI**

Use:

```powershell
rg -n "execution_trail|executionTrail|AssistantRun|assistant run" apps/web/app
```

**Step 2: Display only safe fields**

Show:

- Label.
- Status.
- Denied count.
- Returned count.
- Short user-facing message.

Do not show:

- Full observations.
- Prompt text.
- Hidden reasoning.
- Provider keys.

**Step 3: Add static-page action affordances**

For `update_static_page_module` observations:

- Show a small step like `已生成页面修改建议`.
- Let existing static-page draft operation path apply changes.

For `list_report_options`:

- Show the exact two choices only.

**Step 4: Verify**

Run:

```powershell
npm run build
```

Expected:

- UI compiles.
- Progress display remains concise.

**Commit:** `feat(web): show safe assistant react progress`

### Task 9: Add OpenClaw Bridge Actions Behind The Same Registry

**Files:**

- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/llm-gateway/src/lib.rs` only if provider manifest needs expansion.
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Keep OpenClaw disabled by default**

Environment gates:

- `OPENCLAW_EXTENSION_ENABLED`
- `OPENCLAW_MEMORY_ENABLED`
- `OPENCLAW_READONLY_EXECUTION_ENABLED`

**Step 2: Implement `openclaw_memory_recall`**

Rules:

- V3 selected scope still decides whether memory is relevant.
- OpenClaw memory is labeled `openclaw_memory`.
- It never replaces V3 memory.
- It is capped.

**Step 3: Implement readonly execution stub**

First implementation should return rejected unless explicitly enabled and allowlisted.

**Step 4: Verify**

Run:

```powershell
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api openclaw_react'
```

Expected:

- Disabled OpenClaw does not affect default ReAct behavior.
- Enabled fake bridge returns labeled observations.

**Commit:** `feat(platform-api): add gated openclaw react tools`

### Task 10: Migration Cleanup And De-risking

**Files:**

- Modify: `docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md`
- Modify: `README.md` if runtime env flags change.
- Test: full relevant suite.

**Step 1: Remove duplicate parser/action code from `lib.rs`**

After modules are extracted and tested, keep `lib.rs` as orchestration glue.

**Step 2: Document runtime flags**

Document:

- `ASSISTANT_RUN_REACT_ENABLED`
- `ASSISTANT_RUN_REACT_MAX_STEPS`
- `ASSISTANT_RUN_RUNTIME_PROVIDER`
- `ASSISTANT_RUN_RUNTIME_MODEL`
- OpenClaw gates from Task 9.

**Step 3: Run final verification**

Run:

```powershell
node --test apps/web/app/lib/static-page-draft.test.mjs
npm run build
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all --check'
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react'
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-runtime'
wsl.exe -- bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-renderer'
git diff --check
```

Expected:

- All tests pass.
- No whitespace errors.
- ReAct behavior remains feature-flagged and rollback-safe.

**Commit:** `docs: finalize v3 react refinement handoff`

## 3. Risk Register

- **Risk: Over-abstracting too early.** Keep first registry implementation simple; enum dispatcher is acceptable.
- **Risk: Tool registry becomes hidden business planner.** Registry executes only model-chosen actions and policy repairs; it must not choose the next business action.
- **Risk: Planning catalog leaks answerable content.** Add tests that fail if body/chunk/OCR/table text appears in planning catalog.
- **Risk: Static-page action loop diverges from Q&A contract.** Keep static-page actions as tools under the same protocol, not a separate hidden branch.
- **Risk: Latency increases.** Keep max steps low, cap observations, and show progress in UI.
- **Risk: Fallback hides model/provider failures.** Runtime manifest and trail must record provider/model/fallback reason.

## 4. Recommended Execution Order

1. Task 1 contract extraction.
2. Task 4 protocol repair matrix, because it is the highest safety leverage.
3. Task 3 tool registry extraction.
4. Task 2 weak planning catalog.
5. Task 5 report handoff.
6. Task 6 document detail tool.
7. Task 7 trace.
8. Task 8 frontend progress.
9. Task 9 OpenClaw bridge.
10. Task 10 cleanup.

This order avoids blocking current static-page work while steadily moving V3 toward the Java-reference architecture.

## 5. Acceptance Criteria

- V3 accepts both old `action_type` and new `status/action.type` ReAct shapes during migration.
- Planning catalog cannot be used as evidence.
- Final answers over selected data require observations.
- Report handoff always follows `list_report_options` and returns exact product choices.
- Static-page actions stay model-operable through the same ReAct loop.
- Tool execution remains V3-scoped, allowlisted, capped, and audited.
- Frontend shows progress without chain-of-thought.
- OpenClaw remains optional and cannot bypass V3 policy.
- Existing ordinary chat and deterministic fallback still work.
