# DataMax Codex Host Architecture Alignment Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Align the Codex Host, model proxy, separated memory, workers, and existing DataMax Rust architecture into one non-overlapping module plan.

**Architecture:** DataMax remains the control plane and source of truth. `llm-gateway` remains the provider abstraction and is the seed of a future internal `model-proxy`; Codex Host is an external execution worker, not a model gateway or data authority. Existing workers, tool registry, workflow engine, runtime inspect, PostgreSQL, and artifact publishing stay as reusable DataMax infrastructure instead of being duplicated in the Codex Host path.

**Tech Stack:** Rust workspace crates `platform-api`, `llm-gateway`, `tool-registry`, `mcp-gateway`, `workflow-engine`, `workflow-definitions`, `event-bus`, `storage`, `codex-host-agent`; Next.js web shell in `apps/web`; PostgreSQL 17.9; NATS/worker queue; private Responses-compatible shim only for Codex-to-MiniMax experiments.

---

## 2026-05-18 Freeze Decision

The Codex substrate plan is frozen until the operator explicitly resumes it.

Current verified production status:

- 8 server has Codex CLI installed (`/usr/local/bin/codex`, `codex-cli 0.130.0`) but is not logged in (`codex login status` reports `Not logged in`).
- DataMax ordinary AssistantRun answers still use the provider path, currently `ASSISTANT_RUN_RUNTIME_PROVIDER=minimax` with `MiniMax-M2.7`.
- `aiv3-codex-host-agent` and the local Codex-compatible responses shim are deployed, but the host agent is configured as `CODEX_HOST_AGENT_EXECUTION_MODE=plan_only`, `CODEX_HOST_AGENT_PROFILE_KIND=codex-compatible-shim`, and `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=false`.
- AssistantRun Codex diagnostics remain shadow/plan-only. Real Codex transports are not the authoritative answer path.

Freeze means:

- Do not switch ordinary AssistantRun, external bot, or third-party chat answers from MiniMax/provider routing to Codex-backed routing.
- Do not log the 8 server Codex CLI into the operator's GPT account or store GPT account credentials for DataMax service use.
- Do not enable `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC`, promote `codex_exec_schema`, SDK, app-server, or MCP transports, or add Linux-server real execution support.
- Do not build a new Codex-backed model proxy unless the operator explicitly reopens this track.
- Keep the existing code, services, diagnostics, and documents as a dormant reference. Bug fixes that prevent leakage or accidental execution are allowed; feature work should move back to the current mainline.

If resumed later, restart from three explicit gates: account/login decision, isolated host smoke, and DataMax promotion review. Until then, Codex remains an optional future execution-host design, not the current model gateway.

---

## Why This Plan Exists

The current discussion split the future system into three rough areas:

- Model proxy.
- Codex client / host.
- DataMax project for permissions, datasets, and artifacts.

That direction is right, but DataMax already has a more precise architecture:

- Platform Control Plane.
- Integration Plane: `llm-gateway`, `mcp-gateway`, `tool-registry`, `prompt-registry`.
- Workflow Plane.
- Worker Plane.
- Data / Artifact Plane.
- Experience Layer.

The implementation risk is not that Codex Host is wrong. The risk is accidentally building a second platform around Codex Host and duplicating DataMax's existing control, memory, queue, provider, and artifact responsibilities.

## Final Module Boundary

Use these six modules when discussing, designing, or assigning future work.

### 1. Experience UI

Owns:

- Assistant shell.
- Main workspace.
- Dataset selection display.
- Right shelf for drafts and finished outputs.
- Mobile interaction.

Does not own:

- Dataset visibility decisions.
- Retrieval decisions.
- Model routing.
- Codex Host flags.
- Direct queue or database access.

### 2. DataMax Control Plane

Owns:

- `platform-api`.
- User/session/local-key semantics.
- Dataset and document visibility.
- AssistantRun state.
- Report/static-page draft state.
- Workflow submission.
- Runtime inspect surface.
- Artifact and published output ownership.

Does not own:

- Direct provider-specific HTTP details outside `llm-gateway`.
- Direct local host execution.
- Browser-facing access to Codex app-server or exec-server.

### 3. Integration Plane

Owns:

- `llm-gateway` provider abstraction.
- Future internal `model-proxy` facade around `llm-gateway`.
- `tool-registry` metadata and authorization descriptors.
- `mcp-gateway` external tool boundary.
- `prompt-registry` prompt definitions and prompt rendering policy.

Does not own:

- Final answer composition.
- Retrieval authorization.
- Dataset visibility.
- Conversation memory source of truth.
- Codex task scheduling.

### 4. Workflow / Worker Plane

Owns:

- Explicit workflow definitions.
- Queue task claiming.
- Ingest, retrieval, memory, report, static-page, and media workers.
- Retry, cancel, dead-letter, and replay semantics.

Does not own:

- Browser sessions.
- Provider credential policy.
- Permanent product state outside PostgreSQL repositories.

### 5. External Execution Host

Owns:

- `codex-host-agent`.
- Host profile validation.
- Isolated task workspace.
- Launching `codex exec` only behind explicit safety gates.
- Redacted stdout/stderr summaries.
- Returning artifacts and workflow completion.

Does not own:

- User-facing chat API.
- Dataset visibility.
- Local-key policy.
- System memory.
- Provider credential authority.
- DataMax task scheduler.

### 6. Data / Artifact Plane

Owns:

- PostgreSQL as system of record.
- Object storage assets.
- Qdrant/vector indexes.
- Parquet/DataFusion analytical plane.
- Published static-page/report artifacts.

Does not own:

- UI decisions.
- Model prompt decisions.
- Hidden orchestration logic.

## Conflict Inventory

### Conflict 1: `llm-gateway` vs Future `model-proxy`

Decision:

- Do not create a separate model proxy implementation yet.
- First continue strengthening `llm-gateway`.
- Only create `crates/model-proxy` when at least two independent processes need the same provider facade.

Reason:

- DataMax already has provider abstraction, model lanes, provider error redaction, and output normalization in `llm-gateway`.
- A premature proxy will duplicate model route logic and credential handling.

### Conflict 2: Codex Host vs Worker Plane

Decision:

- Codex Host is one external worker class, not a replacement for DataMax workers.
- Codex tasks must be represented as workflow definitions and queue tasks.

Reason:

- Ingest, retrieval, memory, static-page, report, and media processing already have natural worker ownership.
- Codex is useful for bounded host execution and artifact-producing tasks, not routine data-platform jobs.

### Conflict 3: Codex Host Agent Depending On `platform-api`

Current state:

- `crates/codex-host-agent` depends on `platform-api`.

Decision:

- Accept as a short-term dry-run bridge.
- Refactor after the host contract stabilizes so the agent depends on `contracts`, `workflow-definitions`, `event-bus`, `storage`, and a small shared host-task client instead of `platform-api`.

Reason:

- A host worker linking the whole API crate creates a reverse dependency from execution host back into product HTTP internals.

### Conflict 4: Workers Depending On `platform-api`

Current state:

- Several workers depend on `platform-api`.

Decision:

- Do not block feature work on this immediately.
- Add this as a dependency-cleanup workstream after separated memory and Codex queue semantics are stable.

Reason:

- The original DataMax rule says workers should depend on domain/contracts/storage/event-bus, not web/API handlers.
- This is a creeping giant-module risk.

### Conflict 5: `tool-registry` Depending On `llm-gateway`

Current state:

- `tool-registry` depends on `llm-gateway`.

Decision:

- Review and invert this if the dependency is only for provider/runtime metadata.
- Tool definitions should be independent from model provider execution.

Reason:

- Tool registry describes capabilities and contracts.
- LLM gateway executes model provider calls.
- Keeping them entangled makes a future model proxy harder to extract.

### Conflict 6: OpenClaw, MiniMax Shim, And Codex Host

Decision:

- OpenClaw remains optional provider/legacy sidecar.
- MiniMax chat-to-Responses shim belongs to model gateway/proxy experiments.
- Codex Host remains execution host only.

Reason:

- These three solve different problems.
- Blending them creates unclear ownership of credentials, memory, and provider compatibility.

## Borrowing Rules From DataMax Original Architecture

Borrow these existing DataMax mechanisms before adding anything new:

- Use `workflow-definitions` for every Codex Host task kind.
- Use `workflow-engine` and `event-bus` for task lifecycle instead of a custom Codex scheduler.
- Use `runtime.inspect` for user-visible host progress and operator debugging.
- Use PostgreSQL repositories for durable state and artifact references.
- Use `llm-gateway` lanes for model selection and provider manifests.
- Use `tool-registry` for model-facing tool descriptions and capability allowlists.
- Use `auth-scope` / platform visibility helpers before constructing any Codex task materials.
- Use static-page/report artifact publishing paths for Codex-produced deliverables.

## Strong Architecture Rules

- Browser traffic only talks to DataMax APIs.
- Codex app-server and exec-server are never browser-facing.
- Provider keys are never exposed to browser local storage or Codex task prompts.
- Codex Host receives scoped task materials, not database credentials.
- DataMax validates memory spaces before recall/write/promote.
- DataMax validates dataset visibility before retrieval or Codex task materialization.
- Generated artifacts are not memory unless explicitly promoted.
- `model-proxy` receives already-scoped model input; it does not run RAG or permission checks.
- `codex-host-agent` returns redacted logs and artifact refs; it does not stream raw stdout into UI.

## Task 0: Link This Plan From Active Handoff

**Status 2026-05-07:** completed locally. This plan is created and referenced from the consolidated handoff. Commit remains pending with the broader dirty working tree.

**Files:**

- Modify: `docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md`
- Create: `docs/plans/2026-05-07-DataMax-codex-host-architecture-alignment-plan.md`

**Step 1: Add this plan to Source Plans**

Add:

```markdown
- `docs/plans/2026-05-07-DataMax-codex-host-architecture-alignment-plan.md`: frozen architecture boundary plan aligning DataMax's original control/integration/workflow/worker planes with Codex Host and future model proxy.
```

**Step 2: Add an architecture alignment section**

Add a short section explaining:

- The system has six logical modules, not just three.
- Codex Host is external execution, not model gateway.
- `llm-gateway` is the model gateway seed.
- Existing workers remain the worker plane.

**Step 3: Verify**

Run:

```powershell
git diff --check
```

Expected: no whitespace errors.

**Step 4: Commit**

```powershell
git add docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md docs/plans/2026-05-07-DataMax-codex-host-architecture-alignment-plan.md
git commit -m "docs: align codex host with DataMax architecture"
```

## Task 1: Add Dependency Boundary Audit

**Files:**

- Create: `docs/architecture/DataMax-dependency-boundary-audit.md`
- Optionally create later: `tools/audit-crate-boundaries.ps1`

**Step 1: Record current dependency exceptions**

Document at least:

```text
codex-host-agent -> platform-api
workers -> platform-api
tool-registry -> llm-gateway
llm-gateway -> prompt-registry
```

**Step 2: Classify each exception**

Use these statuses:

```text
accepted_short_term
needs_inversion
needs_shared_service_extraction
needs_no_action
```

**Step 3: Add target dependency rules**

Document target rules:

```text
codex-host-agent -> contracts + workflow-definitions + event-bus + storage + observability
workers -> contracts + domain-model + storage + event-bus + workflow-definitions
tool-registry -> domain/contracts only for metadata
llm-gateway -> prompt-registry only if prompts are rendered inside gateway
```

**Step 4: Verify**

Run:

```powershell
git diff --check
```

Expected: no whitespace errors.

**Step 5: Commit**

```powershell
git add docs/architecture/DataMax-dependency-boundary-audit.md
git commit -m "docs: audit DataMax crate dependency boundaries"
```

## Task 2: Extract Codex Host Shared Contract From `platform-api`

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/workflow-definitions/src/lib.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_tools.rs`
- Test: `crates/codex-host-agent/src/lib.rs`

**Step 1: Write failing contract tests**

Add tests proving the Codex Host task payload round-trips through shared contracts:

```rust
#[test]
fn codex_host_task_payload_round_trips() {
    let payload = CodexHostTaskPayload {
        assistant_run_id: "run-id".to_string(),
        local_thread_id: Some("thread-id".to_string()),
        task_context_id: "codex-task-1".to_string(),
        capability: "inspect_project".to_string(),
        task: "summarize current runtime".to_string(),
        task_memory_policy: serde_json::json!({"kind": "task", "isolated": true}),
        safety: serde_json::json!({"allow_user_flags": false}),
    };

    let encoded = serde_json::to_value(&payload).expect("encode");
    let decoded: CodexHostTaskPayload = serde_json::from_value(encoded).expect("decode");
    assert_eq!(decoded.capability, "inspect_project");
}
```

**Step 2: Move payload structs to `contracts`**

Add shared structs:

```rust
pub struct CodexHostTaskPayload { ... }
pub struct CodexHostTaskObservation { ... }
pub struct CodexHostTaskSafety { ... }
```

**Step 3: Replace ad-hoc JSON construction**

Update `platform-api` ReAct task enqueue logic to construct the shared contract first, then serialize it into workflow context.

**Step 4: Update `codex-host-agent`**

Decode the shared contract from workflow context instead of relying on `platform-api` helpers.

**Step 5: Run focused tests**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p contracts codex_host"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api codex_host"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p codex-host-agent"
```

Expected: all pass.

**Step 6: Commit**

```powershell
git add crates/contracts/src/lib.rs crates/workflow-definitions/src/lib.rs crates/platform-api/src/react_agent_tools.rs crates/codex-host-agent/src/lib.rs
git commit -m "refactor: share codex host task contracts"
```

## Task 3: Reduce `codex-host-agent -> platform-api` Coupling

**Files:**

- Modify: `crates/codex-host-agent/Cargo.toml`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify if needed: `crates/storage/src/lib.rs`
- Test: `crates/codex-host-agent/src/lib.rs`

**Step 1: Identify imported API symbols**

Run:

```powershell
Select-String -LiteralPath crates/codex-host-agent/src/lib.rs -Pattern "platform_api|platform-api|use platform"
```

Expected: list of imports to replace.

**Step 2: Replace API imports with lower-level crates**

Preferred dependencies:

```toml
contracts = { path = "../contracts" }
domain-model = { path = "../domain-model" }
event-bus = { path = "../event-bus" }
observability = { path = "../observability" }
storage = { path = "../storage" }
workflow-definitions = { path = "../workflow-definitions" }
workflow-engine = { path = "../workflow-engine" }
```

**Step 3: Keep behavior unchanged**

Do not add real Codex execution in this task.

**Step 4: Run tests**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p codex-host-agent"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api codex_host"
```

Expected: no behavior change.

**Step 5: Commit**

```powershell
git add crates/codex-host-agent/Cargo.toml crates/codex-host-agent/src/lib.rs
git commit -m "refactor: decouple codex host agent from platform api"
```

## Task 4: Decide `tool-registry` And `llm-gateway` Ownership

**Files:**

- Modify: `docs/architecture/DataMax-dependency-boundary-audit.md`
- Modify if needed: `crates/tool-registry/Cargo.toml`
- Modify if needed: `crates/tool-registry/src/lib.rs`
- Modify if needed: `crates/llm-gateway/src/lib.rs`
- Test: `crates/tool-registry/src/lib.rs`
- Test: `crates/llm-gateway/src/lib.rs`

**Step 1: Inspect why `tool-registry` depends on `llm-gateway`**

Run:

```powershell
Select-String -Path crates/tool-registry/src/*.rs -Pattern "llm_gateway|Llm|Model|Provider"
```

**Step 2: If dependency is only metadata, move metadata**

Move shared metadata to `contracts` or local `tool-registry` types.

**Step 3: If dependency is real execution, document exception**

If tool registry truly invokes model providers, keep the dependency but document why.

**Step 4: Run tests**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p tool-registry"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p llm-gateway"
```

Expected: all pass.

**Step 5: Commit**

```powershell
git add docs/architecture/DataMax-dependency-boundary-audit.md crates/tool-registry crates/llm-gateway crates/contracts
git commit -m "refactor: clarify tool registry model boundaries"
```

## Task 5: Model Proxy Extraction Gate

**Files:**

- Create: `docs/architecture/internal-model-proxy-contract.md`
- Modify: `docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md`
- Test: docs only

**Step 1: Write the extraction gate**

Document that `crates/model-proxy` is created only when one of these is true:

```text
platform-api and at least one worker both need remote provider routing through the same service boundary
Codex Host needs a private Responses-compatible endpoint for non-GPT provider execution
provider credentials/rate limits must be centralized outside the API process
```

**Step 2: Define internal endpoints**

Document:

```text
GET /healthz
GET /v1/model-routes
POST /v1/model-invocations
GET /v1/model-invocations/{id}
POST /v1/responses-compatible/{profile_id}
```

**Step 3: Define non-goals**

Document:

```text
not browser-facing
not customer chat API
not RAG
not memory selector
not dataset visibility authority
not artifact owner
```

**Step 4: Verify**

Run:

```powershell
git diff --check
```

Expected: no whitespace errors.

**Step 5: Commit**

```powershell
git add docs/architecture/internal-model-proxy-contract.md docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md
git commit -m "docs: define internal model proxy extraction gate"
```

## Task 6: Codex Host Real Execution Readiness

**Files:**

- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `docs/operations/codex-host-model-profiles.md`
- Modify: `docs/operations/codex-jump-host-minimax-smoke.md`
- Test: `crates/codex-host-agent/src/lib.rs`

**Step 1: Keep real execution disabled by default**

Confirm these defaults:

```text
CODEX_HOST_AGENT_EXECUTION_MODE=dry_run
CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=false
CODEX_HOST_AGENT_HOST_KIND=developer_workstation
```

**Step 2: Add profile validation tests**

Test cases:

```rust
#[test]
fn real_exec_rejects_developer_workstation() { ... }

#[test]
fn real_exec_requires_explicit_allow_flag() { ... }

#[test]
fn real_exec_requires_execution_capable_profile() { ... }
```

**Step 3: Do not run local-machine Codex**

Validation commands may check code and tests locally, but real `codex exec` validation belongs only on `windows-jump` or later Mac host.

**Step 4: Run tests**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p codex-host-agent"
node --check tools/codex-host-responses-shim-smoke.mjs
```

Expected: local tests pass without launching Codex.

**Step 5: Commit**

```powershell
git add crates/codex-host-agent/src/lib.rs docs/operations/codex-host-model-profiles.md docs/operations/codex-jump-host-minimax-smoke.md tools/codex-host-responses-shim-smoke.mjs
git commit -m "feat: harden codex host execution readiness"
```

## Task 7: Worker Coupling Cleanup Plan

**Files:**

- Create: `docs/plans/2026-05-07-DataMax-worker-boundary-cleanup-plan.md`
- Modify: `docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md`

**Step 1: List worker imports**

Capture current worker dependency exceptions:

```text
chat-session-worker -> platform-api
dataset-output-worker -> platform-api
ingest-worker -> platform-api
memory-worker -> platform-api
report-planner-worker -> platform-api
report-render-worker -> platform-api
retrieval-worker -> platform-api
static-page-worker -> platform-api
```

**Step 2: Split cleanup into later slices**

Do not attempt all workers in one commit.

Recommended order:

```text
codex-host-agent first
static-page-worker second
chat-session-worker third
dataset-output-worker fourth
remaining workers after shared service seams are proven
```

**Step 3: Define extraction targets**

Likely shared crates:

```text
assistant-runtime
static-page-runtime
report-runtime
storage repositories
workflow-definitions
contracts
```

**Step 4: Verify**

Run:

```powershell
git diff --check
```

Expected: no whitespace errors.

**Step 5: Commit**

```powershell
git add docs/plans/2026-05-07-DataMax-worker-boundary-cleanup-plan.md docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md
git commit -m "docs: plan DataMax worker boundary cleanup"
```

## Task 8: End-To-End Architecture Smoke

**Files:**

- Test only unless failures require fixes.

**Step 1: Run format**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all --check"
```

Expected: pass.

**Step 2: Run focused Rust tests**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p llm-gateway"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p workflow-definitions"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api codex_host"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p codex-host-agent"
```

Expected: pass.

**Step 3: Run frontend quick tests if web changed**

Run:

```powershell
node --test apps/web/app/lib/assistant-startup-briefing.test.mjs apps/web/app/lib/scope-planner.test.mjs
pnpm --filter @ai-data-platform-v3/web build
```

Expected: pass.

**Step 4: Run whitespace check**

Run:

```powershell
git diff --check
```

Expected: pass.

## Acceptance Criteria

- A fresh thread can explain the six final modules without reading chat history.
- `model-proxy` is clearly a future service facade over `llm-gateway`, not a new model stack.
- Codex Host is clearly an external execution worker, not a model gateway or control plane.
- Existing workers remain first-class DataMax worker plane components.
- Current dependency exceptions are documented and prioritized.
- No browser path can bypass DataMax API, scope policy, or runtime audit.
- MiniMax/non-GPT Codex experiments are routed through private Responses-compatible shim policy only.
- The plan names exact implementation tasks, files, tests, and commit slices.

## Recommended Next Thread Prompt

```text
Continue DataMax from docs/plans/2026-05-07-DataMax-codex-host-architecture-alignment-plan.md.
Treat this plan as frozen as of 2026-05-18 unless the operator explicitly resumes the Codex substrate track.
Do not promote Codex, log the 8 server Codex CLI into the operator GPT account, enable real transports, or route ordinary AssistantRun/external-bot/third-party chat answers through Codex.
Keep DataMax as the control plane, llm-gateway/provider routing as the active answer path, and Codex Host as dormant diagnostic/reference infrastructure.
Only touch this track for leakage prevention, accidental-execution prevention, or documentation updates while frozen.
```
