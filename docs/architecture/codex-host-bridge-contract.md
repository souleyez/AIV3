# Codex Host Bridge Contract

**Scope:** V3 Rust assistant execution-kernel integration.

Codex Host is an external execution kernel controlled by V3. It is not a browser-facing API, not a dataset visibility authority, and not the system memory source of truth.

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
- `dry_run` records an AssistantRun event named `codex_host_task.dry_run_completed`.
- No local Codex process is launched by the current implementation.

This lets us verify V3 audit, task isolation, queue wakeup, and workflow completion before adding real host execution.

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
- `task_chars`
- optional `local_thread_id`
- `task_memory_isolated`
- optional `task_memory_space_id`

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
- The first transport should be a V3 task queue plus host agent launching `codex exec`, not a browser-visible app-server.

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
```

Supported safe modes right now:

- `dry_run`: complete the workflow and record an event without building a Codex command.
- `plan_only`: validate the profile and capability, then record a redacted command plan with `prompt_redacted=true`.

`codex_exec` can launch `codex exec` only after the safety preflight passes. It requires `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=true`, an approved host kind (`windows_jump` or `mac_host`), and an execution-capable profile kind. The returned process output is truncated and redacted before it enters workflow output or AssistantRun events.

Browser traffic still goes only through V3 APIs. The host agent is a worker attached to the internal workflow queue; it is not a new browser-visible service surface.

## MiniMax Provider Rule

Codex CLI 0.123.0 no longer accepts `wire_api="chat"` providers. MiniMax native `/chat/completions` cannot be configured directly as a Codex provider, and `https://api.minimaxi.com/v1/responses` returned 404 in jump-host validation.

For MiniMax-backed Codex execution, use:

```text
Codex CLI -> private local Responses-compatible shim -> MiniMax /chat/completions
```

The shim must be local/private, must not expose provider keys to browser traffic, and must normalize leading MiniMax `<think>...</think>` blocks before streaming text back to Codex.
