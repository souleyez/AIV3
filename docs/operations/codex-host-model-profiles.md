# Codex Host Model Profiles

**Scope:** V3 Rust assistant Codex Host integration.

Codex Host model selection is a V3 policy decision, not a free-form user prompt field. Users can ask for capability and quality, but V3 maps that request to an approved profile.

## Upstream Codex OSS Baseline

Use upstream `openai/codex` as the execution-kernel baseline:

- Repository: `https://github.com/openai/codex`
- License: Apache-2.0.
- Current observed release on 2026-05-10: `0.130.0`.
- Supported operational surfaces to prefer: `codex exec` for non-interactive work, `--output-schema` for structured final JSON, `@openai/codex-sdk` for server-side TypeScript thread control, app-server JSON-RPC for richer local control, and `codex mcp-server` for tool-style integration.

Do not fork Codex into V3 unless a specific upstream gap blocks these surfaces. Treat a fork as a last-resort patch set, not the main integration path.

## CoDeepSeedeX Reference Pattern

`CoDeepSeedeX` is a useful reference for provider adaptation, not a runtime dependency for V3.

- Repository: `https://github.com/Awenforever/CoDeepSeedeX/tree/master`
- License: MIT.
- Shape: local OpenAI Responses-compatible proxy that lets Codex use DeepSeek-backed profiles.
- Valuable pattern: create Codex profiles that point at local-only provider shims, with separate stable/thinking profiles when the upstream provider exposes different reasoning modes.
- Valuable operations surface: `/healthz`, provider status, balance when available, usage summary/events, debug trace status, and context-budget diagnostics.
- Valuable hardening surface: context trimming, semantic/persistent compaction experiments, tool-output budget reports, protocol repair for tool calls, and liveness recovery when a model stalls mid-tool loop.

V3 should borrow these ideas into its own model-gateway/Codex profile system:

- Implement V3-owned shims only where Codex cannot call a provider natively.
- Keep shims bound to local/private interfaces such as `127.0.0.1`.
- Keep provider keys outside task prompts, browser APIs, user-visible logs, and artifact payloads.
- Persist product-grade usage/audit in PostgreSQL/runtime inspect. Shim-local SQLite or JSONL files are host diagnostics only.
- Treat debug traces as sensitive because they can contain request summaries, paths, tool-output summaries, and usage details.
- Do not let the provider shim execute V3 tools, access datasets, own memory, write queues, or bypass V3 action validation.

## Profile Rules

- Profiles are server-side config only.
- User text cannot pass raw Codex CLI flags.
- Provider keys never enter task prompts or user-visible logs.
- Runtime events record profile id, provider family, model label, and capability class.
- Real execution profiles stay disabled until tested on `windows-jump` or the later Mac host.
- The local developer workstation must not be used for Codex execution tests.
- Task prompts must not include provider keys, browser-local keys, or raw secrets.
- Each host task runs under a V3-created `task_memory_space_id`; the host can return summaries, but V3 decides whether anything is promoted into conversation or project memory.
- Codex transport is an explicit profile field. User prompts cannot switch between CLI, SDK, app-server, or MCP.

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
transport = "exec_schema"
model = "gpt-5.3-codex"
allowed_capabilities = ["inspect_project", "summarize_runtime", "run_readonly_check"]

[profiles.codex-native-readonly]
kind = "codex-native"
enabled = false
transport = "exec_schema"
model = "gpt-5.3-codex"
allowed_capabilities = ["inspect_project", "summarize_runtime", "run_readonly_check"]

[profiles.minimax-private-experiment]
kind = "codex-compatible-shim"
enabled = false
transport = "exec_schema"
provider = "minimax"
model = "MiniMax-M2.7"
wire_api = "responses"
base_url = "http://127.0.0.1:<private-shim-port>/v1"
allowed_capabilities = ["inspect_project", "summarize_runtime"]

[profiles.codex-threaded-assistant]
kind = "codex-native"
enabled = false
transport = "sdk_thread"
model = "gpt-5.3-codex"
allowed_capabilities = ["assistant_conversation", "static_page_plan", "static_page_edit"]

[profiles.deepseek-private-reference]
kind = "codex-compatible-shim"
enabled = false
transport = "exec_schema"
provider = "deepseek"
model = "deepseek-v4-pro"
wire_api = "responses"
base_url = "http://127.0.0.1:<private-shim-port>/v1"
allowed_capabilities = ["inspect_project", "summarize_runtime"]
```

Allowed transport values:

```text
exec_schema -> codex exec with a V3-owned output schema for one-shot worker tasks
sdk_thread  -> @openai/codex-sdk thread control for continuing AssistantRun conversations
app_server  -> host-local app-server JSON-RPC for richer control
mcp_server  -> codex mcp-server when Codex should be exposed as a controlled tool
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

First production-leaning path:

```text
assistant chat / static-page action -> V3 AssistantRun context package
  -> profile transport=exec_schema or sdk_thread
  -> Codex receives only V3-supplied context/tool contracts
  -> Codex returns structured response/action intent
  -> V3 validates and executes requested action
```

`codex_exec` must require all of these before it can be wired:

```text
CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=true
CODEX_HOST_AGENT_HOST_KIND=windows_jump|mac_host
CODEX_HOST_AGENT_PROFILE_KIND=codex-native|codex-compatible-shim
CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT=<host-local task workspace root>
profile capability allowlist contains the requested capability
task context includes a V3-created assistant_run_id and task_memory_space_id
```

`codex_exec` output policy:

```text
output is serialized as CodexHostTaskOutputView
AssistantRun event is codex_host_task.exec_completed
stdout/stderr excerpts are truncated
secret-looking log lines are replaced with [redacted-log-line]
the full task prompt is never included in command_plan summaries
Codex runs from a task-scoped workspace label derived from task_memory_space_id
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

If MiniMax must be used as the Codex model itself, the profile must target a private Responses-compatible shim. Do not configure MiniMax Chat Completions directly as a Codex provider until upstream Codex supports that provider shape.

## Provider-Shim Observability Contract

Every V3-owned Codex-compatible provider shim should expose enough diagnostics for the platform to decide whether a profile is healthy before routing a conversation through it:

```text
health                  -> process and upstream reachability
status                  -> profile id, provider family, model, capabilities, rate-limit hints
usage_summary           -> prompt/completion/total tokens or provider equivalents
usage_events            -> recent redacted upstream calls for runtime inspect
balance                 -> optional, only when provider supports safe account balance lookup
debug_trace_status      -> whether trace capture is enabled and where redacted trace ids live
context_budget_report   -> request payload budget by system, memory, datasets, evidence, tools, artifacts
tool_output_budget      -> largest tool outputs and trimming decisions
liveness_events         -> retry/continue decisions for incomplete tool-call loops
```

These diagnostics must be redacted, bounded, and linked to V3 `AssistantRun` or workflow ids when possible. They are for operations and quality control; they are not user-facing answer content.

Current V3-side foundation:

- `contracts` defines a Provider Shim observability snapshot with health, profile/capability snapshot, usage summary/events, optional balance, debug-trace status, context-budget report, tool-output budget, and liveness events.
- `llm-gateway` can build a safe profile/status snapshot from `ModelProviderProfile` without exposing raw provider keys or raw base URLs.
- `llm-gateway` can also convert provider runtime metadata into redacted Provider Shim usage events and summaries, ready for V3 runtime inspect or future PostgreSQL audit persistence.
- AssistantRun detail responses now include safe diagnostics for the latest Codex shadow comparison, context-budget pressure, jump-host/Mac-host validation readiness, completed Codex Host validation output summaries with `host_kind`, and redacted provider usage events. V3 treats completed host validation as valid only when it comes from `windows_jump` or `mac_host`, giving audit/runtime views a stable read path without scanning raw event payloads in the browser.
- Real Codex conversation transports are still double-gated: without `ASSISTANT_RUN_CODEX_REAL_TRANSPORT_FEATURE_GATE=enabled`, V3 records the requested transport but runs the effective Codex executor in `codex_plan_only` and exposes the downgrade in `transport_policy`. Enabling that feature gate only lets the requested transport reach `assistant-runtime`; current runtime behavior still returns an unsupported-transport blueprint with `codex_invoked=false` until host validation and real transport wiring are implemented.
- AssistantRun Codex context packages now carry a tool-output budget policy; V3 only trims oversized `tool_outputs` payload fields and preserves evidence refs, source locators, media timestamps, and error/status details.
- The snapshot is a contract for future local shim/host diagnostics. It is not yet a browser-facing route and does not grant the shim access to V3 tools, datasets, queues, or memory.

## Contract Boundary

The worker consumes workflow context generated from `CodexHostTaskRequestView` and records workflow output shaped as `CodexHostTaskOutputView`. These shared contracts live in `crates/contracts` so `codex-host-agent` can stay independent of `platform-api`.

The worker dependency direction should stay narrow:

```text
codex-host-agent -> contracts + workflow-definitions + workflow-engine + storage + event-bus
codex-host-agent -X-> platform-api
browser -X-> codex-host-agent
```

This keeps browser authentication, dataset visibility, report/static-page APIs, and user-facing orchestration inside V3 while allowing Codex Host to remain an optional execution extension.
