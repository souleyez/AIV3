# Codex Host Bridge Contract

**Scope:** V3 Rust assistant execution-kernel integration.

Codex Host is an external execution kernel controlled by V3. It is not a browser-facing API, not a dataset visibility authority, and not the system memory source of truth.

## OpenAI Codex OSS Alignment

V3 should align with upstream `openai/codex` instead of inventing a parallel execution kernel.

Checked reference on 2026-05-10:

- Repository: `https://github.com/openai/codex`
- License: Apache-2.0.
- Current observed release: `0.130.0` on 2026-05-08.
- Primary implementation is Rust-heavy, with CLI, Rust core, SDKs, docs, scripts, and tooling.
- Install/runtime surfaces include npm, Homebrew, GitHub release binaries, `codex exec`, app-server, SDKs, and MCP server mode.

Integration decision:

- Do not vendor or fork Codex in V3 first.
- Use official runtime surfaces as the contract boundary.
- Keep V3's Rust `codex-host-agent` as the queue/worker wrapper that prepares a task workspace, generates Codex config/profile, launches or controls Codex, and returns redacted structured output.
- Keep a local checkout of `openai/codex` only for debugging SDK/app-server behavior, building a pinned binary on a host, or evaluating a patch before upstreaming.

## CoDeepSeedeX Alignment

`CoDeepSeedeX` is a useful provider-shim reference for this bridge because it shows how Codex can call a non-OpenAI provider through a local Responses-compatible proxy.

Checked reference on 2026-05-10:

- Repository: `https://github.com/Awenforever/CoDeepSeedeX/tree/master`
- License: MIT.
- Shape: local OpenAI Responses-compatible proxy for Codex plus DeepSeek profiles.
- Useful ideas: provider profile wrappers, local-only health/status/usage/debug endpoints, context-budget diagnostics, tool-output trimming, tool-call protocol repair, liveness recovery, and MCP/tool boundary warnings.

Boundary decision:

- V3 can borrow the provider-shim pattern for MiniMax, DeepSeek, or other non-native providers.
- V3 should not copy the monolithic proxy architecture wholesale.
- A provider shim only normalizes model API traffic for Codex. It is not a V3 API, not an execution worker, not a database reader, not a queue owner, and not a tool permission authority.
- MCP and V3 tool execution remain controlled by Codex/V3 action contracts and allowlists. Do not map arbitrary MCP namespace tools into plain provider function tools as a shortcut.
- Shim-local debug files and usage ledgers are diagnostics only; V3 PostgreSQL/runtime inspect remains the durable audit source.

## Boundary

```text
V3 AssistantRun/ReAct
  -> V3 validates scope, memory space, capability, and allowlist
  -> V3 creates an audited task context
  -> Codex Host Agent runs Codex in an isolated task workspace
  -> artifacts, summaries, and redacted logs return to V3
```

First implementation stays disabled by default.

## Current Implementation

The first V3-side implementation is intentionally only a queue bridge plus dry-run worker:

- ReAct can emit `codex_host_task`, but `CODEX_HOST_TASK_ENABLED` defaults to false.
- When enabled and allowlisted, V3 creates `codex_host_task_workflow` and enqueues `codex_host/run_codex_host_task`.
- `crates/codex-host-agent` can claim that queue task and complete it in `dry_run` mode.
- `plan_only` mode can build a redacted Codex command plan without launching Codex.
- Shared request/result wire shapes live in `crates/contracts`, not in `platform-api`.
- `crates/codex-host-agent` advances workflow state through storage, workflow definitions, and event bus dependencies. It must not depend on the browser-facing API crate.
- AssistantRun events are mode-specific: `codex_host_task.dry_run_completed`, `codex_host_task.plan_only_completed`, or `codex_host_task.exec_completed`.
- AssistantRun detail responses now expose safe diagnostics for Codex executor shadow events, jump-host/Mac-host validation readiness, completed Codex Host validation outputs, and provider usage, so runtime/audit surfaces can show status without exposing raw prompts, provider keys, command arguments, or verbose host logs.
- Browser-facing AssistantRun execution treats `codex_exec_schema`, `codex_sdk_thread`, `codex_app_server`, and `codex_mcp_server` as requested transports only. Until the manual feature gate is explicitly enabled after shadow plus allowed-host validation, V3 records fixed-field `transport_policy`, `host_invocation`, and `suggested_action` summaries and downgrades the effective transport to `codex_plan_only`.
- No local Codex process is launched unless the worker is explicitly switched to `codex_exec` and passes the host/profile safety preflight.
- Real `codex_exec` also requires a configured task workspace root. The agent creates a task-scoped workspace from the V3 `task_memory_space_id` and runs Codex from that directory instead of the agent's current working directory.

This lets us verify V3 audit, task isolation, queue wakeup, and workflow completion before adding real host execution.

Next implementation should add explicit transports:

- `exec_schema`: one-shot `codex exec` with `--output-schema` so V3 receives stable JSON summaries/action intents.
- `sdk_thread`: server-side `@openai/codex-sdk` control for continuing Codex threads when a long-lived AssistantRun needs continuity.
- `app_server`: local app-server JSON-RPC for richer local control where SDK coverage is insufficient.
- `mcp_server`: only if V3 needs to expose Codex as a tool inside another MCP/agent framework.

Transport choice is a V3 policy field, not user text.

## Shared Contract Types

Codex Host queue context should be produced through `CodexHostTaskRequestView` and host output should serialize through `CodexHostTaskOutputView`.

The request contract owns these fields:

- `assistant_run_id`
- `capability`
- optional bounded `task`
- optional `local_thread_id`
- `task_memory_policy`
- top-level `task_memory_space_id`
- `safety`

The output contract owns these fields:

- `mode`
- `codex_invoked`
- `status`
- `assistant_run_id`
- `capability`
- optional safe `profile`, `command_plan`, and `process` summaries
- `command_plan.workspace_configured` and a safe `workspace_label`, never the raw task prompt
- `task_chars`
- optional `local_thread_id`
- `task_memory_isolated`
- optional `task_memory_space_id`
- optional `html_artifacts` for safe V3-owned review surfaces such as `codex_execution_report`

AssistantRun diagnostics may summarize this output as `codex_executor.host_validation_results`, but that summary must stay bounded and redacted:

- include mode, status, capability, profile kind/model/provider, workspace configured/label, exit code, and log character counts
- exclude raw command arguments, provider auth env names, stdout/stderr excerpts, provider keys, and raw prompts
- include `host_kind` and count a completed `codex_exec` smoke as valid only when it comes from `windows_jump` or `mac_host`
- keep `direct` execution authoritative until shadow comparison and allowed jump-host/Mac-host validation both pass

All worker modes must serialize their successful output through `CodexHostTaskOutputView`. This includes `codex_exec`; real process output is represented only by safe profile, command-plan, process summaries, and sandboxable HTML artifact manifests.

`html_artifacts` are not raw model HTML. They are V3-owned manifests with a template id, owner scope, provenance, interaction mode, and sanitized payload. The browser renders them through the safe HTML artifact viewer, not as arbitrary app code. Platform API persists trusted manifests into the `html_artifacts` table and still reads AssistantRun event payloads as a compatibility fallback/backfill path. Interactive static-page planning handoffs execute only by translating safe JSON Patch payloads into existing static-page operations or by sending a natural-language action intent through the static-page intent interpreter; they do not mutate arbitrary JSON paths or database rows.

`task_memory_space_id` is intentionally duplicated at the top level and inside `task_memory_policy.memory_space_id` so queue workers, UI observations, and future memory storage do not need to parse nested policy JSON just to route task-local recall.

## Workflow Contract

```text
workflow_kind=codex_host_task_workflow
queue=codex_host
task_key=run_codex_host_task
success_stage=codex_host_task_completed
```

The first ReAct integration only enqueues a workflow task after all of these are true:

- `CODEX_HOST_TASK_ENABLED=true`
- `CODEX_HOST_TASK_ALLOWLIST` contains the requested capability
- The action is attached to an existing AssistantRun

The queued workflow context carries:

- `assistant_run_id`
- `local_thread_id` when available
- `capability`
- bounded task text from `arguments.task`, `arguments.prompt`, or `arguments.instruction`
- `task_memory_policy` with isolated task memory
- top-level `task_memory_space_id`
- safety flags forbidding user-controlled CLI flags and secrets in prompts

## ReAct Action

```json
{
  "action_type": "codex_host_task",
  "reason_summary": "why this host task is needed",
  "arguments": {
    "capability": "inspect_project",
    "task": "bounded task description"
  },
  "requires_confirmation": false
}
```

Default observation:

```json
{
  "status": "rejected",
  "action_type": "codex_host_task",
  "message": "codex_host_execution_disabled",
  "items": [],
  "limits": {}
}
```

## Required Safety Rules

- `CODEX_HOST_TASK_ENABLED` defaults to false.
- `CODEX_HOST_TASK_ALLOWLIST` is required even when enabled.
- A task must name a capability before any host execution can be considered.
- User text cannot set Codex CLI flags directly.
- Codex task memory is isolated from normal conversation memory.
- Every queued Codex Host task gets a task-scoped memory space id. Summaries can be promoted later only through V3 policy, not by the host process.
- Provider keys and local access keys are never sent to the Codex task prompt.
- Raw stdout/stderr must be redacted and truncated before being shown in runtime inspect.
- The first transport should be a V3 task queue plus host agent launching `codex exec --output-schema`, not a browser-visible app-server.
- App-server, SDK, and MCP transports must bind only to host-local/private surfaces and remain hidden behind V3 APIs.
- Codex config is generated from approved V3 profiles. User text must not set `sandbox_mode`, `approval-policy`, model profile, writable roots, MCP servers, or environment policy.

## Jump Host Rule

Until the Mac host is ready, `windows-jump` may be used for non-destructive smoke tests. Do not run local-machine Codex from the developer workstation for this project.

## Worker Environment

```text
PLATFORM_DATABASE_URL=postgres://...
PLATFORM_NATS_URL=nats://...
CODEX_HOST_QUEUE=codex_host
CODEX_HOST_TASK_KEY=run_codex_host_task
CODEX_HOST_AGENT_POLL_INTERVAL_MS=1000
CODEX_HOST_AGENT_EXECUTION_MODE=dry_run
CODEX_HOST_AGENT_PROFILE_ID=default-dry-run
CODEX_HOST_AGENT_PROFILE_KIND=dry-run
CODEX_HOST_AGENT_PROFILE_MODEL=
CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES=
CODEX_HOST_AGENT_HOST_KIND=developer_workstation
CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=false
CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT=
```

Supported safe modes right now:

- `dry_run`: complete the workflow and record an event without building a Codex command.
- `plan_only`: validate the profile and capability, then record a redacted command plan with `prompt_redacted=true`.
- `codex_exec`: launch `codex exec` only after host/profile/allowlist safety preflight, then record a shared-contract output with redacted stdout/stderr excerpts.

Planned transport modes:

- `exec_schema`: launch `codex exec` with a V3-owned output schema and parse only the final JSON response as actionable data.
- `sdk_thread`: use Codex SDK thread control for continuing runs; persist the Codex thread id only as internal worker metadata.
- `app_server`: use local app-server JSON-RPC for richer control; never expose app-server to browser traffic.
- `mcp_server`: use `codex mcp-server` only for controlled framework integration, not as the primary V3 browser API.

`codex_exec` can launch `codex exec` only after the safety preflight passes. It requires `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=true`, an approved host kind (`windows_jump` or `mac_host`), an execution-capable profile kind, and `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT` pointing at a host-local task workspace root. The returned process output is truncated and redacted before it enters workflow output or AssistantRun events.

Browser traffic still goes only through V3 APIs. The host agent is a worker attached to the internal workflow queue; it is not a new browser-visible service surface.

## MiniMax Provider Rule

Codex CLI 0.123.0 no longer accepts `wire_api="chat"` providers. MiniMax native `/chat/completions` cannot be configured directly as a Codex provider, and `https://api.minimaxi.com/v1/responses` returned 404 in jump-host validation.

For MiniMax-backed Codex execution, use:

```text
Codex CLI -> private local Responses-compatible shim -> MiniMax /chat/completions
```

The shim must be local/private, must not expose provider keys to browser traffic, and must normalize leading MiniMax `<think>...</think>` blocks before streaming text back to Codex.

## Provider Shim Rule

Any future V3-owned Responses-compatible shim must obey the same bridge boundary:

```text
Codex CLI/SDK/app-server -> private provider shim -> upstream provider API
V3 API/workers          -> AssistantRun, datasets, memory, workflows, artifacts, audit
```

The shim may expose redacted diagnostics to V3:

- health and provider status
- profile/capability snapshot
- usage summary and recent usage events
- context-budget and tool-output budget reports
- liveness/protocol-repair events

The shared V3 contract for this exists as `ProviderShimObservabilitySnapshotView` in `crates/contracts`. `llm-gateway` can already derive the safe profile portion from `ModelProviderProfile`; future shim/host endpoints should fill runtime health, usage, balance, debug trace, budget, and liveness fields from bounded/redacted local diagnostics. AssistantRun Codex packages also carry a tool-output budget policy so oversized execution payloads can be bounded without dropping retrieval evidence, refs, media timestamps, or failure details.

The shim must not expose:

- provider keys
- raw prompts containing user secrets
- raw tool outputs beyond bounded/redacted excerpts
- direct V3 database access
- direct queue submission
- direct dataset/file access
- browser-visible endpoints
