# Codex Host Model Profiles

**Scope:** V3 Rust assistant Codex Host integration.

Codex Host model selection is a V3 policy decision, not a free-form user prompt field. Users can ask for capability and quality, but V3 maps that request to an approved profile.

## Profile Rules

- Profiles are server-side config only.
- User text cannot pass raw Codex CLI flags.
- Provider keys never enter task prompts or user-visible logs.
- Runtime events record profile id, provider family, model label, and capability class.
- Real execution profiles stay disabled until tested on `windows-jump` or the later Mac host.
- The local developer workstation must not be used for Codex execution tests.
- Task prompts must not include provider keys, browser-local keys, or raw secrets.
- Each host task runs under a V3-created `task_memory_space_id`; the host can return summaries, but V3 decides whether anything is promoted into conversation or project memory.

## Initial Profiles

```toml
[profiles.default-dry-run]
kind = "dry-run"
enabled = true
allowed_capabilities = ["inspect_project", "summarize_runtime"]

[profiles.plan-readonly]
kind = "codex-native"
enabled = true
mode = "plan_only"
model = "gpt-5.3-codex"
allowed_capabilities = ["inspect_project", "summarize_runtime", "run_readonly_check"]

[profiles.codex-native-readonly]
kind = "codex-native"
enabled = false
model = "gpt-5.3-codex"
allowed_capabilities = ["inspect_project", "summarize_runtime", "run_readonly_check"]

[profiles.minimax-private-experiment]
kind = "codex-compatible-shim"
enabled = false
provider = "minimax"
model = "MiniMax-M2.7"
wire_api = "responses"
base_url = "http://127.0.0.1:<private-shim-port>/v1"
allowed_capabilities = ["inspect_project", "summarize_runtime"]
```

## Execution Policy

The first real execution mode should be read-only:

```text
capability=inspect_project
workspace=leased task workspace
network=disabled unless explicitly needed
write_access=false
artifact_upload=true
raw_logs=redacted and truncated
```

Current Worker modes:

```text
dry_run   -> no Codex command is built
plan_only -> build a redacted Codex command plan, but do not launch Codex
codex_exec -> launch `codex exec` only after host/profile/allowlist safety preflight
```

`codex_exec` must require all of these before it can be wired:

```text
CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=true
CODEX_HOST_AGENT_HOST_KIND=windows_jump|mac_host
CODEX_HOST_AGENT_PROFILE_KIND=codex-native|codex-compatible-shim
profile capability allowlist contains the requested capability
task context includes a V3-created assistant_run_id and task_memory_space_id
```

`codex_exec` output policy:

```text
stdout/stderr excerpts are truncated
secret-looking log lines are replaced with [redacted-log-line]
the full task prompt is never included in command_plan summaries
non-zero Codex exit marks the workflow step failed
```

Write-capable Codex Host tasks need a separate capability and a stronger approval policy:

```text
capability=propose_patch
workspace=isolated branch or copied worktree
write_access=true
commit_access=false by default
human_review_required=true
```

## MiniMax Experiment Boundary

The existing jump-host MiniMax smoke only proves direct MiniMax API reachability. It does not prove that Codex CLI can use MiniMax as its own provider.

2026-05-07 jump-host finding:

```text
Codex CLI 0.123.0 rejects wire_api="chat".
MiniMax native https://api.minimaxi.com/v1/responses returns 404.
Therefore Codex cannot use MiniMax directly through Chat Completions config.
Codex can use a local Responses-compatible shim, and that shim can call MiniMax /chat/completions.
```

To make MiniMax valid for Codex Host, one of these must be true:

- Codex natively supports the configured provider shape.
- V3 exposes a private, local-only Codex-compatible provider shim.
- Codex task uses V3 tools that call `llm-gateway`, while Codex itself keeps its native model.

The first production path should prefer V3-owned `llm-gateway` for MiniMax and keep Codex Host focused on execution.

## Contract Boundary

The worker consumes workflow context generated from `CodexHostTaskRequestView` and records workflow output shaped as `CodexHostTaskOutputView`. These shared contracts live in `crates/contracts` so `codex-host-agent` can stay independent of `platform-api`.

The worker dependency direction should stay narrow:

```text
codex-host-agent -> contracts + workflow-definitions + workflow-engine + storage + event-bus
codex-host-agent -X-> platform-api
browser -X-> codex-host-agent
```

This keeps browser authentication, dataset visibility, report/static-page APIs, and user-facing orchestration inside V3 while allowing Codex Host to remain an optional execution extension.
