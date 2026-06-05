# Codex Gateway And Model Proxy Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Separate Codex execution-host integration from DataMax model routing, so DataMax can use Codex as the Mac execution kernel without pretending Codex provides a stable production model gateway.

**Architecture:** Codex is treated as an execution runtime, not as the product's model gateway. DataMax owns model routing, provider credentials, policy, memory scope, audit, and runtime manifests through `llm-gateway` and a future optional `model-proxy` service. Codex Mac Host is controlled through a DataMax task bridge, while models are called through DataMax-owned provider adapters unless a tightly scoped experimental Codex-compatible shim is explicitly enabled.

**Tech Stack:** Rust `llm-gateway`, `platform-api`, `workflow-definitions`, `workflow-engine`, future `codex-host-agent`; Next.js web only consumes DataMax APIs; Codex CLI/app-server/exec-server/MCP surfaces are local host runtime details; PostgreSQL 17.9 stores provider/model/runtime audit.

---

## Direct Answer

Do not assume Codex has a product-grade gateway proxy for DataMax.

Local Codex exposes useful runtime surfaces:

- `codex exec`: non-interactive execution.
- `codex app-server`: experimental app-server with `stdio://` or WebSocket listen modes.
- `codex exec-server`: experimental standalone WebSocket service.
- `codex mcp-server`: stdio MCP server.
- `codex --remote`: client connects to a remote app-server WebSocket.
- `codex --model`, `--oss`, `--local-provider`: model/provider selection at Codex CLI level.

These are not the same thing as a stable DataMax model gateway:

- They are Codex runtime/control surfaces, not a multi-tenant provider gateway.
- `app-server` and `exec-server` are explicitly experimental.
- They do not own DataMax dataset visibility, separated memory, report/static-page policy, local-key access, or runtime audit.
- They should not be exposed directly to browsers or customers.

Therefore the plan is:

```text
Codex = execution kernel / Mac workstation runtime
DataMax llm-gateway/model-proxy = model gateway and provider policy
DataMax platform-api/workflows = business state, memory, permissions, audit, artifacts
```

## Boundary Decision

### What Codex Should Do

- Execute bounded tasks on the Mac host.
- Use local files/tools/browser only inside DataMax-approved task scope.
- Produce artifacts, logs, summaries, and patches.
- Run with isolated `task_context_id` and task memory.
- Optionally use Codex's own model configuration for execution quality.

### What Codex Should Not Do

- Act as the public API gateway for customer chat.
- Decide dataset visibility or local-key access.
- Hold product-level provider credentials for all users.
- Read PostgreSQL directly for business data.
- Serve a browser-facing WebSocket/HTTP endpoint.
- Become the source of truth for memory.

### What DataMax Model Gateway Should Do

- Route AssistantRun/static-page/report/parser model calls.
- Hide provider keys from browser and Codex task prompts.
- Normalize provider responses into DataMax `LlmResponse`.
- Record provider, model, request id, token usage, finish reason, and redacted errors.
- Apply model capability rules: text, vision, audio, video, long-context, JSON reliability, tool-call reliability.
- Support OpenClaw as optional provider, not main execution kernel.

## Target Architecture

```mermaid
flowchart LR
  "DataMax Web" --> "DataMax Platform API"
  "DataMax Platform API" --> "AssistantRun / ReAct"
  "AssistantRun / ReAct" --> "Memory Scope Policy"
  "AssistantRun / ReAct" --> "LLM Gateway"
  "LLM Gateway" --> "OpenAI Provider"
  "LLM Gateway" --> "MiniMax Provider"
  "LLM Gateway" --> "Claude/Gemini Provider Later"
  "LLM Gateway" --> "OpenClaw Provider Optional"
  "AssistantRun / ReAct" --> "Workflow Queue"
  "Workflow Queue" --> "Codex Host Agent"
  "Codex Host Agent" --> "Codex CLI / app-server"
  "Codex Host Agent" --> "Task Workspace"
  "Codex Host Agent" --> "Artifacts / Redacted Logs"
  "Artifacts / Redacted Logs" --> "Runtime Inspect"
```

## Three-Layer Plan

### Layer 1: DataMax `llm-gateway`

This is the first and safest layer. It already exists.

Responsibilities:

- Provider abstraction.
- OpenClaw provider already implemented.
- Add model profile registry.
- Add model capability registry.
- Add redacted provider error handling.
- Add routing policy for AssistantRun, static-page intent, VLM/media parsing, and image prompt summarization.

This layer is called directly by `platform-api`, `static-page-runtime`, `document-vlm-runtime`, workers, and later the model proxy service.

### Layer 2: Optional DataMax `model-proxy` Service

Only add this after in-process `llm-gateway` becomes too crowded.

Responsibilities:

- HTTP/RPC facade around the same provider registry.
- Centralized credential loading.
- Rate limiting.
- Provider health checks.
- Model fallback policy.
- Invocation audit.

First version should be internal-only:

```text
127.0.0.1 or private network only
no browser access
service token required
all errors redacted
```

Do not create a public OpenAI-compatible endpoint in the first version. That encourages accidental bypass of DataMax policy.

### Layer 3: Codex Host Bridge

This is not a model gateway.

Responsibilities:

- DataMax creates a signed/leased task.
- Codex host agent picks up the task.
- Codex runs locally with task-specific workspace and memory.
- Result artifacts return to DataMax.
- Runtime inspect shows safe progress.

If Codex needs to use a different model, prefer one of these routes:

1. Use Codex's supported local/provider flags when launching the task.
2. Let the task call DataMax-approved tools that themselves call `llm-gateway`.
3. Only later experiment with a Codex-compatible provider shim if absolutely necessary.

## Model Proxy Strategy

### Phase 1: In-Process Model Routing

Keep model routing inside Rust `llm-gateway`.

Add:

```rust
pub struct ModelRoute {
    pub lane: String,
    pub provider: String,
    pub model: String,
    pub capability_class: Vec<String>,
    pub priority: i32,
    pub fallback_route: Option<String>,
}
```

Example lanes:

```text
assistant_chat
assistant_react_json
static_page_intent
static_page_image_prompt
document_vlm
audio_transcript
video_scene_summary
report_planning
codex_task_summary
```

Acceptance:

- Every provider call records lane/provider/model.
- No caller hardcodes OpenClaw/MiniMax/OpenAI except gateway setup.
- Provider failure returns redacted runtime facts.

### Phase 2: Internal `model-proxy` Binary

Add a Rust binary only when needed:

```text
crates/model-proxy/src/main.rs
```

Internal endpoints:

```text
GET  /healthz
GET  /v1/model-routes
POST /v1/model-invocations
GET  /v1/model-invocations/{id}
```

Do not implement customer-facing chat semantics here. It receives already-scoped model input from DataMax.

### Phase 3: Codex Provider Configuration

Codex Host Agent can launch Codex with controlled config:

```text
codex exec -C <task-workspace> --model <profile-model> --ask-for-approval never --sandbox workspace-write "<task prompt>"
```

For local model experiments:

```text
codex exec --oss --local-provider ollama ...
```

Policy:

- DataMax chooses allowed model profile.
- Codex Host Agent maps profile to CLI/config.
- User prompt cannot directly set arbitrary model/provider flags.
- Logs record the selected profile, not secrets.
- Real host validation may run on the `windows-jump` machine before the Mac host is ready.
- Do not use the abandoned `codex-web`/remote bridge direction for this project.
- Do not use any public or legacy remote front door as the Codex model provider path for DataMax.
- First non-GPT validation must use a local-only or private-network MiniMax path controlled by DataMax.
- If Codex CLI itself must use a non-OpenAI model through DataMax, the endpoint must be a real Codex/OpenAI Responses-compatible provider shim, not a workstation-control bridge API.

### 2026-05-07 Jump Host Scope Correction And MiniMax Finding

Project boundary:

- `codex-web` and public remote bridge experiments are a separate abandoned project direction.
- DataMax Rust assistant work must not depend on that path.
- Jump-host validation is only for the DataMax assistant Codex-kernel direction.
- The allowed first test path is private MiniMax API access from the jump host.

Read-only checks on `windows-jump` showed:

- OS: Windows 11.
- Node: `v22.22.1`.
- npm: `10.9.4`.
- Codex CLI: `codex-cli 0.123.0`.
- MiniMax `/chat/completions` smoke succeeded from the jump host using `MiniMax-M2.7`.
- MiniMax may return leading `<think>...</think>` content in `message.content`; DataMax must strip or isolate this before user-facing answer rendering.
- Codex CLI 0.123.0 rejects `wire_api="chat"` providers.
- MiniMax native `https://api.minimaxi.com/v1/responses` returned 404.
- A jump-host local Responses-compatible shim successfully let Codex call MiniMax and return `CODEX_HOST_SMOKE_OK`.

Interpretation:

- The jump host is a valid place for real-machine Codex CLI and private MiniMax testing.
- The MiniMax key and base URL should be treated as server-side secrets only.
- For DataMax's "Codex as execution kernel" plan, keep using Codex as an execution host and keep model routing in DataMax.
- For the "Codex uses MiniMax/non-GPT" experiment, direct Chat Completions config is not viable on Codex 0.123.0. Build or expose a dedicated private Responses-compatible provider shim.

### Phase 4: Experimental Codex-Compatible Provider Shim

Only consider this if we really need Codex itself to speak through DataMax's model proxy.

Risk:

- Codex may rely on provider/tool semantics that are not fully OpenAI-compatible.
- A fake Responses-compatible shim can silently degrade tool calling, patch quality, streaming, or safety.

If implemented, it must be explicitly experimental:

```text
CODEX_PROVIDER_SHIM_ENABLED=false
CODEX_PROVIDER_SHIM_LISTEN=127.0.0.1 only
single Mac host only
no browser access
full request/response redaction
limited model profiles
```

## Security Rules

- Browser never calls model proxy directly.
- Codex app-server/exec-server is never public.
- Non-loopback Codex app-server requires signed bearer/capability token.
- Provider keys live in server/host secret stores, not localStorage.
- Codex task prompts receive scoped materials, not database credentials.
- Dataset private visibility is enforced before model input is built.
- Memory spaces are selected before model input is built.
- Provider errors are redacted before runtime manifest/UI.
- Raw Codex stdout/stderr is stored as artifact/log only after redaction and truncation.

## Task 0: Link This Plan From Active Plans

**Files:**

- Modify: `docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md`
- Modify: `docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md`
- Add: `docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md`

**Step 1: Add source-plan references**

Add this plan as the detailed source for model proxy/Codex host boundary.

**Step 2: Verify**

Run:

```powershell
git diff --check
```

Expected: no whitespace errors.

**Step 3: Commit**

```powershell
git add docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md
git commit -m "docs: split codex gateway model proxy plan"
```

## Task 1: Add Model Route Registry

**Status 2026-05-07:** implemented. `llm-gateway` now has model lane constants, `ModelRoute`, `ModelRouteRegistry`, env override support, and `resolve_runtime_selection_from_env`. AssistantRun can use lane-level overrides such as `LLM_GATEWAY_ROUTE_ASSISTANT_CHAT_PROVIDER=minimax_openai_compatible` while preserving old `ASSISTANT_RUN_RUNTIME_*` fallback behavior.

**Files:**

- Modify: `crates/llm-gateway/src/lib.rs`
- Test: `crates/llm-gateway/src/lib.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[test]
fn model_route_registry_selects_lane_default() {
    let registry = ModelRouteRegistry::from_env_with_defaults("assistant_chat");
    let route = registry.select("assistant_chat").expect("route");
    assert!(!route.provider.is_empty());
    assert!(!route.model.is_empty());
}

#[test]
fn model_route_registry_rejects_unknown_lane_without_default() {
    let registry = ModelRouteRegistry::empty();
    assert!(registry.select("missing").is_none());
}
```

**Step 2: Implement minimal structs**

Add:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRoute {
    pub lane: String,
    pub provider: String,
    pub model: String,
    pub capability_class: Vec<String>,
    pub priority: i32,
    pub fallback_route: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ModelRouteRegistry {
    routes: Vec<ModelRoute>,
}
```

**Step 3: Add lane constants**

```rust
pub const MODEL_LANE_ASSISTANT_CHAT: &str = "assistant_chat";
pub const MODEL_LANE_ASSISTANT_REACT_JSON: &str = "assistant_react_json";
pub const MODEL_LANE_STATIC_PAGE_INTENT: &str = "static_page_intent";
pub const MODEL_LANE_DOCUMENT_VLM: &str = "document_vlm";
pub const MODEL_LANE_AUDIO_TRANSCRIPT: &str = "audio_transcript";
pub const MODEL_LANE_VIDEO_SCENE_SUMMARY: &str = "video_scene_summary";
```

**Step 4: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p llm-gateway model_route"
```

Expected: tests pass.

## Task 2: Route Existing Provider Calls Through Lanes

**Status 2026-05-07:** partially implemented. AssistantRun chat/ReAct, static-page intent, chat-session worker, and dataset-output worker now attach lane metadata to `LlmRequest` and runtime manifests. AssistantRun also resolves chat and ReAct runtime independently by lane. Remaining follow-up: apply lane runtime selection to static-page intent and media/document lanes once their concrete MiniMax provider configs are finalized.

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/static-page-runtime/src/lib.rs`
- Modify: `crates/document-vlm-runtime/src/lib.rs`
- Test: relevant existing crate tests

**Step 1: Add lane field to provider runtime manifest**

Every model call should record:

```json
{
  "lane": "assistant_react_json",
  "provider": "openai|openclaw|minimax|...",
  "model": "...",
  "request_id": "..."
}
```

**Step 2: Replace direct env naming gradually**

Current env flags like:

```text
ASSISTANT_RUN_RUNTIME_PROVIDER
STATIC_PAGE_INTENT_RUNTIME_PROVIDER
```

remain backward compatible, but internally map to lanes.

**Step 3: Tests**

Add focused tests:

```rust
#[tokio::test]
async fn assistant_run_runtime_manifest_records_model_lane() { ... }

#[test]
fn static_page_intent_uses_static_page_lane() { ... }
```

**Step 4: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_runtime"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-runtime provider"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p document-vlm-runtime"
```

Expected: existing model calls still work and runtime manifests include lanes.

## Task 3: Redact Provider Errors

**Status 2026-05-07:** implemented in `llm-gateway` for OpenAI-compatible and OpenClaw provider error paths. HTTP error bodies are redacted and truncated before entering provider errors/runtime manifests.

**Files:**

- Modify: `crates/llm-gateway/src/lib.rs`
- Test: `crates/llm-gateway/src/lib.rs`

**Step 1: Write tests**

```rust
#[test]
fn provider_error_redacts_bearer_tokens() {
    let redacted = redact_provider_error("Authorization: Bearer secret-token");
    assert!(!redacted.contains("secret-token"));
}

#[test]
fn provider_error_truncates_large_body() {
    let redacted = redact_provider_error(&"x".repeat(10_000));
    assert!(redacted.len() < 1000);
}
```

**Step 2: Implement redaction**

Cover at least:

- bearer tokens
- api keys
- access tokens
- cookies
- secret-looking local paths

**Step 3: Apply to OpenClaw provider**

Do not include raw HTTP body in errors.

**Step 4: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p llm-gateway redacts"
```

Expected: no token-like content appears in error strings.

## Task 3A: Normalize Provider Output Reasoning Blocks

**Status 2026-05-07:** implemented in `llm-gateway` after jump-host MiniMax smoke showed `MiniMax-M2.7` can return leading `<think>...</think>` blocks inside `message.content`.

**Files:**

- Modify: `crates/llm-gateway/src/lib.rs`
- Test: `crates/llm-gateway/src/lib.rs`

**Rules:**

- Strip only leading `<think>...</think>` blocks from provider text.
- Do not remove inline literal text that appears later in the answer.
- Apply to OpenAI-compatible chat completion output and OpenClaw Responses/chat fallback output.
- Keep this as output normalization, not prompt engineering. Provider prompts may still ask for concise answers, but the gateway must protect user-facing text.

**Verify:**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p llm-gateway output_normalization"
```

## Task 4: Define Codex Host Bridge Contract

**Status 2026-05-07:** first safe queue contract implemented. The ReAct contract now recognizes `codex_host_task`, planning catalog exposes Codex Host as disabled, tool execution rejects it by default with `codex_host_execution_disabled`, and allowlist checks are in place. `codex_host_task_workflow` is registered with queue `codex_host` and task key `run_codex_host_task`; when enabled in a continued AssistantRun, DataMax creates an audited workflow execution and enqueues the task instead of running Codex inline. `crates/codex-host-agent` now claims that queue, supports `dry_run`, supports `plan_only`, and can launch `codex exec` only when host/profile/real-exec safety gates pass. Local workstation Codex execution remains blocked by default.

**Files:**

- Create: `docs/architecture/codex-host-bridge-contract.md`
- Modify: `crates/platform-api/src/react_agent_contract.rs`
- Modify: `crates/platform-api/src/react_agent_catalog.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Test: `crates/platform-api/src/react_agent_tools.rs`

**Step 1: Document Codex host transport options**

Document:

```text
Preferred first version: DataMax task queue + codex-runtime-agent launches codex exec.
Possible later version: codex app-server over authenticated local/private WebSocket.
Avoid first version: exposing codex app-server or exec-server directly to DataMax Web/browser.
```

**Step 2: Add disabled ReAct action**

Add:

```text
codex_host_task
```

Default result:

```json
{
  "status": "rejected",
  "reason": "codex_host_execution_disabled"
}
```

**Step 3: Add tests**

```rust
fn codex_host_task_is_disabled_by_default() { ... }
fn codex_host_task_rejects_unsafe_capability() { ... }
```

**Step 4: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api codex_host"
```

Expected: safe disabled behavior passes.

**Additional 2026-05-07 verification:**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p workflow-definitions codex_host && cargo test -p platform-api codex_host -- --nocapture"
```

Expected: Codex Host workflow starts on the dedicated queue, and ReAct preflight remains disabled/allowlisted.

## Task 5: Design Future Internal Model Proxy Binary

**Files:**

- Create: `docs/architecture/internal-model-proxy-contract.md`
- Create later only when needed: `crates/model-proxy/Cargo.toml`
- Create later only when needed: `crates/model-proxy/src/main.rs`

**Step 1: Document contract first**

Define internal endpoints:

```text
GET /healthz
GET /v1/model-routes
POST /v1/model-invocations
GET /v1/model-invocations/{id}
```

**Step 2: Explicitly forbid browser usage**

Document:

```text
The model proxy accepts only scoped model input from DataMax services.
It is not a customer chat API.
It does not run retrieval.
It does not decide memory.
It does not decide dataset visibility.
```

**Step 3: Defer implementation**

Do not build `crates/model-proxy` until at least two independent processes need the same provider gateway.

## Task 6: Codex Model Selection Policy

**Status 2026-05-07:** documented initial profile policy in `docs/operations/codex-host-model-profiles.md`. Current safe modes are dry-run and plan-only. `codex_exec` requires explicit real-exec opt-in, approved host kind, and profile capability allowlist before launching. MiniMax private experiment execution profiles remain disabled until jump-host or Mac-host validation.

**Files:**

- Create: `docs/operations/codex-host-model-profiles.md`
- Modify later: `codex-runtime-agent` config once that agent exists

**Step 1: Define model profiles**

Example:

```toml
[profiles.default-codex]
kind = "codex-native"
model = "gpt-5.3-codex"

[profiles.local-ollama]
kind = "codex-oss"
local_provider = "ollama"
enabled = false

[profiles.readonly-fast]
kind = "codex-native"
model = "gpt-5.3-codex-spark"
allowed_capabilities = ["inspect_runtime", "run_readonly_check"]
```

**Step 2: Policy**

- DataMax chooses the profile.
- User text cannot set arbitrary CLI flags.
- Profile choice is recorded in runtime manifest.
- Secrets are not logged.

## Task 7: Jump Host MiniMax And Codex-Kernel Smoke Harness

**Files:**

- Create: `docs/operations/codex-jump-host-minimax-smoke.md`
- Modify later: `codex-runtime-agent` config once that agent exists

**Step 1: Document current host state**

Record:

```text
host_alias=windows-jump
host_os=Windows 11
codex_version=0.123.0
node_version=22.22.1
npm_version=10.9.4
codex_home=C:\Users\soulz\.codex
minimax_smoke_model=MiniMax-M2.7
minimax_smoke_result=ok
```

**Step 2: Define safe MiniMax smoke**

Use only one-shot process environment injection on the jump host:

```powershell
POST https://api.minimaxi.com/v1/chat/completions
model=MiniMax-M2.7
prompt="Reply exactly MINIMAX_SMOKE_OK"
```

Never print the API key. Never write the key into jump-host Codex config during smoke tests.

**Step 3: Define safe Codex CLI smoke**

Use only non-destructive probes:

```powershell
codex --version
codex exec --help
```

Do not run local-machine Codex from the development workstation. Codex CLI validation belongs on the jump host or later Mac host.

**Step 4: Track MiniMax reasoning-block behavior**

If MiniMax returns:

```text
<think>...</think>
final answer
```

then `llm-gateway` must strip or isolate the leading reasoning block before the answer reaches AssistantRun/static-page/report user-facing text.

**Step 5: Add shim acceptance for non-GPT providers**

The MiniMax/non-GPT experiment is only considered valid when:

- Codex CLI can complete the minimal smoke prompt through a DataMax-owned or private provider endpoint.
- The endpoint implements the Responses semantics Codex needs, including streaming/event shape if Codex requires it.
- The endpoint redacts provider errors.
- The endpoint records selected model profile, provider, request id, and lane.
- The endpoint is not reachable from browsers or public customer traffic.

## Acceptance Criteria

- The plan states clearly that Codex does not replace DataMax's model gateway.
- DataMax owns model provider routing through `llm-gateway`.
- Codex Host is an execution bridge, not a browser-facing gateway.
- Future `model-proxy` is internal-only and optional.
- OpenClaw remains provider/legacy sidecar.
- Provider errors are redacted.
- Codex model selection is profile-based and controlled by DataMax.

## Recommended Next Thread Prompt

```text
Continue from docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md.
Treat Codex as execution runtime, not as product model gateway.
Continue Codex Host after the implemented dry-run queue bridge.
Next: add a real host-execution mode in crates/codex-host-agent behind a disabled-by-default profile allowlist, and validate only on windows-jump or the later Mac host.
Keep browser traffic going only through DataMax APIs.
Do not expose Codex app-server/exec-server directly to users.
Do not run local-machine Codex from the developer workstation.
```
