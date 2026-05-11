# Jump Host MiniMax Smoke Notes

**Scope:** AI Data Platform V3 Rust assistant Codex-kernel work only.

This note deliberately excludes the abandoned `codex-web` remote bridge direction. Jump-host validation here means private MiniMax/Codex Host validation for V3, not public remote Codex routing.

## Host Snapshot

```text
host_alias=windows-jump
host_os=Windows 11
node_version=22.22.1
npm_version=10.9.4
codex_version=codex-cli 0.123.0
codex_home=C:\Users\soulz\.codex
```

## MiniMax Smoke Result

```text
date=2026-05-07
base_url=https://api.minimaxi.com/v1
endpoint=/chat/completions
model=MiniMax-M2.7
result=ok
expected_text=MINIMAX_SMOKE_OK
```

The API key was injected only into the remote process environment for the smoke run. It was not printed and was not written into jump-host Codex config.

Observed behavior:

```text
MiniMax-M2.7 can return leading <think>...</think> text inside message.content.
```

V3 must normalize this before user-facing answer rendering. `llm-gateway` should strip or isolate leading reasoning blocks for OpenAI-compatible provider output.

## Codex Provider Smoke Result

```text
date=2026-05-07
codex_version=codex-cli 0.123.0
direct_minimax_chat_provider=failed
direct_minimax_responses_provider=failed
local_responses_shim_fake=ok
local_responses_shim_to_minimax=ok
expected_text=CODEX_HOST_SMOKE_OK
```

Findings:

- `wire_api="chat"` is rejected by Codex CLI 0.123.0.
- `wire_api="responses"` against `https://api.minimaxi.com/v1` fails because MiniMax has no `/responses` endpoint there.
- A local private Responses-compatible shim can satisfy Codex and translate the request to MiniMax `/chat/completions`.
- The smoke used `--ignore-user-config` so the jump host's old `souleye-cloud` Codex config was not used.
- The MiniMax key was passed through process stdin/environment for the smoke and was not written into Codex config.

Reusable smoke tool:

```text
tools/codex-host-responses-shim-smoke.mjs
```

## Safe Smoke Rules

- Do not run local-machine Codex for this project validation.
- Use `windows-jump` now, and later the dedicated Mac host.
- Do not write MiniMax keys into browser local storage or Codex task prompts.
- Do not expose MiniMax provider endpoints to browsers.
- Do not reuse abandoned public remote bridge routes for V3.
- If Codex itself needs MiniMax later, build a private Responses-compatible shim and validate it separately.

## Codex Host Agent Smoke Procedure

Use this only on the jump host or later Mac host. Do not run these commands on the developer workstation.

The smoke has two stages:

1. `plan_only`: verifies the V3 workflow context, profile allowlist, task memory isolation, workspace label, and redacted command plan without launching Codex.
2. `codex_exec`: launches Codex only after `plan_only` is clean and the host has an isolated workspace root plus an explicit real-exec allow flag.

Minimal `plan_only` environment:

```powershell
$env:CODEX_HOST_AGENT_EXECUTION_MODE = "plan_only"
$env:CODEX_HOST_AGENT_HOST_KIND = "windows_jump"
$env:CODEX_HOST_AGENT_PROFILE_ID = "jump-minimax-shim"
$env:CODEX_HOST_AGENT_PROFILE_KIND = "codex-compatible-shim"
$env:CODEX_HOST_AGENT_PROFILE_MODEL = "MiniMax-M2.7"
$env:CODEX_HOST_AGENT_PROFILE_PROVIDER_ID = "minimax"
$env:CODEX_HOST_AGENT_PROFILE_WIRE_API = "responses"
$env:CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES = "inspect_project,code_review"
$env:CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT = "D:\codex-host\tasks"
```

Only after the command plan is redacted and the task workspace label is correct, enable real execution:

```powershell
$env:CODEX_HOST_AGENT_EXECUTION_MODE = "codex_exec"
$env:CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC = "true"
```

Expected Host-side guardrails:

- `CODEX_HOST_AGENT_HOST_KIND` must be `windows_jump` or `mac_host`.
- `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC` must be `true` for `codex_exec`.
- `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT` must be configured; the worker must run from a task-scoped workspace label, not from the agent's current directory.
- Profile kind must be `codex-native` or `codex-compatible-shim`.
- Capability must be in `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES`.

Expected V3 diagnostic closure:

- AssistantRun detail diagnostics should keep `codex_executor.shadow_gate.host_validation.ready_for_jump_host_validation=true` only after stable matched shadow runs.
- Browser-facing AssistantRun diagnostics should show any requested real transport downgrade in `codex_executor.latest.transport_policy` until the manual real-transport feature gate is enabled.
- Completed jump-host output should appear under `codex_executor.host_validation_results`.
- A successful smoke result should have `mode=codex_exec`, `status=completed`, `host_kind=windows_jump` or `host_kind=mac_host`, `host_validation_completed=true`, `codex_invoked=true`, `command_plan.workspace_configured=true`, and `process.exit_code=0`.
- A completed `codex_exec` result from `developer_workstation` or an unknown host kind must be treated as `invalid_host`, not as a valid promotion signal.
- If shadow comparison is stable but the latest `codex_exec` smoke fails, `codex_executor.promotion_gate.status` must remain `blocked_by_host_validation`; mutation and queue submission must still be `false`.
- The diagnostics must not expose command arguments, provider auth env names, raw stdout/stderr excerpts, provider keys, or raw prompts.

If `host_validation_results` is empty, first check whether the Codex Host task completed as `codex_host_task.exec_completed` or a workflow completion event containing a `CodexHostTaskOutputView` shaped `output`.
