# Codex Host Bridge Contract

**Scope:** DataMax Rust assistant execution-kernel integration.

Codex Host is an external execution kernel controlled by DataMax. It is not a browser-facing API, not a dataset visibility authority, and not the system memory source of truth.

## OpenAI Codex OSS Alignment

DataMax should align with upstream `openai/codex` instead of inventing a parallel execution kernel.

Checked reference on 2026-05-10:

- Repository: `https://github.com/openai/codex`
- License: Apache-2.0.
- Current observed release: `0.130.0` on 2026-05-08.
- Primary implementation is Rust-heavy, with CLI, Rust core, SDKs, docs, scripts, and tooling.
- Install/runtime surfaces include npm, Homebrew, GitHub release binaries, `codex exec`, app-server, SDKs, and MCP server mode.

Integration decision:

- Do not vendor or fork Codex in DataMax first.
- Use official runtime surfaces as the contract boundary.
- Keep DataMax's Rust `codex-host-agent` as the queue/worker wrapper that prepares a task workspace, generates Codex config/profile, launches or controls Codex, and returns redacted structured output.
- Keep a local checkout of `openai/codex` only for debugging SDK/app-server behavior, building a pinned binary on a host, or evaluating a patch before upstreaming.

## CoDeepSeedeX Alignment

`CoDeepSeedeX` is a useful provider-shim reference for this bridge because it shows how Codex can call a non-OpenAI provider through a local Responses-compatible proxy.

Checked reference on 2026-05-10:

- Repository: `https://github.com/Awenforever/CoDeepSeedeX/tree/master`
- License: MIT.
- Shape: local OpenAI Responses-compatible proxy for Codex plus DeepSeek profiles.
- Useful ideas: provider profile wrappers, local-only health/status/usage/debug endpoints, context-budget diagnostics, tool-output trimming, tool-call protocol repair, liveness recovery, and MCP/tool boundary warnings.

Boundary decision:

- DataMax can borrow the provider-shim pattern for MiniMax, DeepSeek, or other non-native providers.
- DataMax should not copy the monolithic proxy architecture wholesale.
- A provider shim only normalizes model API traffic for Codex. It is not a DataMax API, not an execution worker, not a database reader, not a queue owner, and not a tool permission authority.
- MCP and DataMax tool execution remain controlled by Codex/DataMax action contracts and allowlists. Do not map arbitrary MCP namespace tools into plain provider function tools as a shortcut.
- Shim-local debug files and usage ledgers are diagnostics only; DataMax PostgreSQL/runtime inspect remains the durable audit source.

## Boundary

```text
DataMax AssistantRun/ReAct
  -> DataMax validates scope, memory space, capability, and allowlist
  -> DataMax creates an audited task context
  -> Codex Host Agent runs Codex in an isolated task workspace
  -> artifacts, summaries, and redacted logs return to DataMax
```

First implementation stays disabled by default.

## Current Implementation

The first DataMax-side implementation is intentionally only a queue bridge plus dry-run worker:

- ReAct can emit `codex_host_task`, but `CODEX_HOST_TASK_ENABLED` defaults to false.
- When enabled and allowlisted, DataMax creates `codex_host_task_workflow` and enqueues `codex_host/run_codex_host_task`.
- `crates/codex-host-agent` can claim that queue task and complete it in `dry_run` mode.
- `plan_only` mode can build a redacted Codex command plan without launching Codex.
- Shared request/result wire shapes live in `crates/contracts`, not in `platform-api`.
- `crates/codex-host-agent` advances workflow state through storage, workflow definitions, and event bus dependencies. It must not depend on the browser-facing API crate.
- AssistantRun events are mode-specific: `codex_host_task.dry_run_completed`, `codex_host_task.plan_only_completed`, or `codex_host_task.exec_completed`.
- AssistantRun detail responses now expose safe diagnostics for Codex executor shadow events, jump-host/Mac-host validation readiness, completed Codex Host validation outputs, and provider usage, so runtime/audit surfaces can show status without exposing raw prompts, provider keys, command arguments, or verbose host logs.
- Browser-facing AssistantRun execution treats `codex_exec_schema`, `codex_sdk_thread`, `codex_app_server`, and `codex_mcp_server` as requested transports only. Until both the manual real-transport feature gate and the manual promotion-review gate are explicitly enabled after shadow plus allowed-host validation, DataMax records fixed-field `transport_policy`, `host_invocation`, and `suggested_action` summaries and downgrades the effective transport to `codex_plan_only`.
- No local Codex process is launched unless the worker is explicitly switched to `codex_exec` and passes the host/profile safety preflight.
- Real `codex_exec` also requires a configured task workspace root. The agent creates a task-scoped workspace from the DataMax `task_memory_space_id` and runs Codex from that directory instead of the agent's current working directory.
- Real `codex_exec` is bounded by runtime controls:
  - `CODEX_HOST_AGENT_TASK_TIMEOUT_MS`, default `900000`
  - `CODEX_HOST_AGENT_HEARTBEAT_MS`, default `15000`
  - `CODEX_HOST_AGENT_STDOUT_LIMIT_BYTES`, default `200000`
  - `CODEX_HOST_AGENT_STDERR_LIMIT_BYTES`, default `100000`
- The worker uses async process execution with timeout. Timeout, launch errors, non-zero exits, fixed-output parse failures, and cancellation-before/during-run are mapped to bounded failure reasons. Raw stdout/stderr are never written into AssistantRun events; only character counts and redacted report manifests are retained.
- While a real process is running, the worker may append `codex_host_task.exec_heartbeat` events. Heartbeats include status, elapsed time, profile/command summaries, and redaction flags, but no raw prompts, stdout, stderr, provider logs, or secrets.

This lets us verify DataMax audit, task isolation, queue wakeup, and workflow completion before adding real host execution.

Next implementation should add explicit transports:

- `exec_schema`: one-shot `codex exec` with `--output-schema` so DataMax receives stable JSON summaries/action intents.
- `sdk_thread`: server-side `@openai/codex-sdk` control for continuing Codex threads when a long-lived AssistantRun needs continuity.
- `app_server`: local app-server JSON-RPC for richer local control where SDK coverage is insufficient.
- `mcp_server`: only if DataMax needs to expose Codex as a tool inside another MCP/agent framework.

Transport choice is a DataMax policy field, not user text.

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
- optional `html_artifacts` for safe DataMax-owned review surfaces such as `codex_execution_report`

AssistantRun diagnostics may summarize this output as `codex_executor.host_validation_results`, but that summary must stay bounded and redacted:

- include mode, status, capability, profile kind/model/provider, workspace configured/label, exit code, and log character counts
- exclude raw command arguments, provider auth env names, stdout/stderr excerpts, provider keys, and raw prompts
- include `host_kind` and count a completed `codex_exec` smoke as valid only when it comes from `windows_jump` or `mac_host`
- expose model readiness separately as `codex_executor.model_gateway_gate`; promotion review stays blocked unless the Codex conversation profile is present, authenticated, and supports a Codex-compatible or JSON-action surface
- keep `direct` execution authoritative until shadow comparison, model readiness, and allowed jump-host/Mac-host validation all pass

All worker modes must serialize their successful output through `CodexHostTaskOutputView`. This includes `codex_exec`; real process output is represented only by safe profile, command-plan, process summaries, and sandboxable HTML artifact manifests.

`html_artifacts` are not raw model HTML. They are DataMax-owned manifests with a template id, owner scope, provenance, interaction mode, and sanitized payload. The browser renders them through the safe HTML artifact viewer, not as arbitrary app code. Platform API persists trusted manifests into the `html_artifacts` table and still reads AssistantRun event payloads as a compatibility fallback/backfill path. Interactive static-page planning handoffs execute only by translating safe JSON Patch payloads into existing static-page operations or by sending a natural-language action intent through the static-page intent interpreter; they do not mutate arbitrary JSON paths or database rows.

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

Advanced static-page work can use this bridge with `capability=static_page_advanced_publish` after allowlisting. The task should receive only a bounded DataMax static-page package: requirement summary, draft/render ids, safe data-source summaries, artifact paths or generated-artifact URLs, and the expected output schema. The host may propose口径 repairs, HTML artifact edits, or publish steps, but DataMax remains responsible for applying writes, publishing to `/generated-artifacts/`, and recording audit.

## Customer Web Codex Router

Customer-facing complex requests are routed through Codex Host as a DataMax-controlled executor lane, not as arbitrary repo patch requests. This lets customers use Codex from the web UI while DataMax keeps task memory, workspace, and publication boundaries explicit. The default capability is `customer_complex_request`, which runs read-only and can return a structured answer, action intent, or handoff plan while the normal AssistantRun answer path remains available.

The web router is a general customer Codex executor, not a static-page-only editor. Static-page editing is one specialized customer artifact path. General customer analysis, planning, data work, report generation, document/package/script creation, and artifact revision should be admitted into Codex Host whenever the capability and profile are allowlisted; only V3 product/source/deploy changes are blocked for operator review.

Customer-owned artifacts may use a writable isolated workspace:

- `customer_artifact_request` may run with `workspace-write` only inside an isolated customer task workspace under `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT`.
- `generated_static_page_edit` may run with `workspace-write` only in a task workspace seeded with a V3-generated static page artifact.
- `generated_static_page_publish` may run with `workspace-write` only in a task workspace scoped by DataMax selected-scope/evidence context; when DataMax has a static-page package, it may be provided as an additional seed.
- DataMax validates generated-artifact scope, data refs, manifests, redaction, and publication state before any user-visible artifact or static page version is published.
- `generated_static_page_edit` workflow context should include `workspace_seed.schema=v3.codex_host_workspace_seed` with the current artifact's generated-artifact URL and safe static-page brief. The host agent materializes that seed into `workspace-seed.json`, `task.json`, and `existing-artifact/` when the source generated artifact exists on the host. `generated_static_page_publish` is used for explicit new static-page/webpage/dashboard generation requests and can start from the selected scope and evidence summary without an existing-artifact seed.

Writable customer tasks must describe their produced files through a workspace-local manifest:

- The preferred manifest path is `customer-artifact-manifest.json`; `artifacts/manifest.json` and `generated-artifacts/manifest.json` are accepted for compatibility.
- Manifest file paths must be relative to the task workspace. Absolute paths, `..`, symlink escapes, `.git`, `node_modules`, `.env`, and credential-like files are rejected by the host agent.
- Manifest paths must not point at reserved workspace inputs such as `existing-artifact/`, `workspace-seed.json`, `task.json`, `runtime.json`, `schemas/`, `evidence/`, or Image2 seed assets.
- For `generated_static_page_edit`, when `existing-artifact/` was materialized, the host agent must validate the submitted manifest before publication: the output must include an HTML page artifact, at least one output file must be comparable to the seeded current page package, and at least one comparable file must differ from `existing-artifact/`. A byte-identical republish of the current page is rejected before copying to `/generated-artifacts/`.
- When `CODEX_HOST_AGENT_CUSTOMER_ARTIFACT_PUBLISH_ENABLED` is unset or true, the host agent may copy only the already validated manifest-listed files into the generated-artifact root under `customer-codex/{capability}/{assistant_run_id}/{workflow_execution_id}/...`. Setting the flag to `false` keeps the task as a workspace-only handoff.
- Published customer artifact output includes only public `/generated-artifacts/` URLs plus safe metadata; it must not expose absolute task workspace paths or the generated-artifact filesystem root.
- Host output appends a safe `customer_artifacts` object to `codex_host_task.exec_completed` and emits a dedicated assistant-run event:
  - `assistant_run.customer_artifact_request_artifacts_ready`
  - `assistant_run.generated_static_page_edit_artifacts_ready`
- `generated_static_page_publish` intentionally uses the generic `assistant_run.customer_artifact_request_artifacts_ready` event name, but the payload must include `capability=generated_static_page_publish` and `route=generated_static_page_publish`; Platform API must preserve those fields when projecting the event into output artifacts.
- Platform API projects those ready events into `AssistantRunDetailView.run.output_artifacts` as `type=codex_customer_artifact_bundle`, with a `v3.output_artifact_manifest` that contains only relative artifact paths, metadata, size, sha256, publication flags, and generated-artifact URLs that pass the DataMax URL allowlist.
- Platform API must allow only artifact-producing capabilities/routes in projected artifact bundles: `customer_artifact_request`, `generated_static_page_edit`, and `generated_static_page_publish`. Non-artifact capabilities such as `customer_complex_request` and `v3_product_change_request` must not be projected as customer artifact bundles. The web normalizer may still show task cards for the broader customer Codex status set: `customer_complex_request`, `customer_artifact_request`, `generated_static_page_edit`, `generated_static_page_publish`, and `v3_product_change_request`. Unknown or unsafe routes must fall back to the validated capability/default route, not be displayed verbatim.
- Browser-visible and runtime-inspect-visible customer Codex display text, including artifact title/summary/file title, task failure reason, and permission scope, must be dropped or replaced with a fixed safe fallback when it resembles credentials, tokens, database URLs, raw prompt/log text, absolute local paths, `/srv/aiv3/repo`, `/srv/aiv3/shared`, `.env`, or other internal paths.
- Platform API and the web UI must reject arbitrary external URLs and pending generated-artifact placeholders. Only `/generated-artifacts/...` or the configured generated-artifact public base URL may be surfaced as `public_url` / `primary_url`.
- The web UI may poll assistant-run detail after likely customer Codex requests and render both task status cards and artifact bundles in the right-side Codex shelf. `customer_complex_request` can be visible as a read-only Codex execution even when it produces no files. Workspace-only bundles remain `published=false` and show a manifest-ready state; host-published bundles show `published=true` and may expose an "open artifact" link plus safe per-file generated-artifact links.
- `customer_complex_request` and other customer Web Codex capabilities may attach `customer_result_summary` to `codex_host_task.exec_completed`. This is the only browser-facing Codex result surface for no-file tasks. The host must rebuild it from structured final JSON, not from raw stdout/stderr. For `codex_exec`, the host should materialize `schemas/customer-result-summary.schema.json` in the task workspace and pass `--output-schema schemas/customer-result-summary.schema.json` unless `CODEX_HOST_AGENT_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED=false` is set for an older CLI. The accepted schema is `v3.customer_codex_result_summary` with bounded `title`, `summary`, `findings`, `recommended_next_actions`, `warnings`, `artifact_intent`, `capability`, and safety flags. The host and web normalizer must drop summaries that expose raw logs, credentials, absolute local paths, prompt text, database URLs, cookies, or token-like strings.
- The dedicated event is still a DataMax-owned artifact lifecycle signal. DataMax owns persistence, preview, publication policy, redaction, versioning, and rollback.

Customer traffic must not mutate the V3 product itself. Requests to change V3 source code, services, migrations, auth, public APIs, provider configuration, systemd/nginx/database settings, commits, deployment, or rollback are classified as `v3_product_change_request` and require operator review. The Platform API should record `assistant_run.codex_sidecar_scope_blocked` with `status=needs_operator_review`, `reason=v3_product_change_not_customer_writable`, `v3_product_repo_write_allowed=false`, and `customer_writable=false` instead of silently routing the request to a writable task. The normal AssistantRun answer path remains available but must tell the customer that operator review is required. The Codex Host agent should reject that capability even if it is accidentally allowlisted by a profile.

## Static Page Template Governance

Static-page reports are governed as reusable customer templates, not disposable pages. Once a dataset/default-prompt combination has an accepted static-page baseline, normal report/page requests should reuse that baseline unless the customer explicitly asks for a redesign, new style, new effect image, or from-scratch page.

Current routing rules:

- Delivery/view requests such as "send the previous report link" or "open the latest report" return the accepted generated-artifact link without creating a new draft, Image2 job, or Codex publish workflow.
- Default create/refresh requests for the same dataset, such as "生成新百经营分析月报，按当前数据刷新并保留分店筛选", reuse the accepted baseline by default and expose `reuse_reason=default_dataset_template_reuse`.
- Concrete revision requests such as "修改已有报表", "修复联动", "补充面积/坪效", or "删除模块" should keep the accepted baseline as the visual contract and route to a controlled Codex update path.
- Explicit redesign requests such as "重新设计", "换风格", "全新页面", or "从头做" may bypass the baseline and create a new candidate/effect-image flow.
- Template reuse events must expose safe governance metadata such as `reuse_policy=reuse_accepted_baseline_unless_explicit_redesign`, `reuse_reason`, `default_template_reuse`, `template_match_policy`, and `dataset_artifact_key` when available.

## Fixed Task Templates

Some Cloudflare Codex work is narrow enough to run without per-task human confirmation after the operator approves the template policy once. These tasks must use server-owned fixed template packages, not arbitrary user prompts.

Current fixed templates:

- `static_page_image2_data_publish`
- `answer_quality_autofix`

Fixed-template rules:

- `template_id` is mandatory in the workflow context.
- `capability` must match the template id and be allowlisted by both DataMax and the host profile.
- Raw task text is optional and bounded; DataMax-owned structured fields are authoritative.
- Host output is accepted only when it matches the template output schema.
- Direct host permissions to DataMax or the 8 server do not bypass DataMax validation, artifact path checks, write-scope checks, or audit.
- Human confirmation is still required when a template instance requests actions outside its no-confirm policy.

`static_page_image2_data_publish` may create and publish a new generated artifact only when `publish_mode=new_generated_artifact_only`, the artifact path is under `/generated-artifacts/`, and the output contains snapshot/date/unit口径 validation. It must not overwrite stable URLs or alter source code without human confirmation.

`answer_quality_autofix` may create diagnosis and low-risk answer-quality patch proposals only inside its allowlisted files/symbols. It must not deploy automatically or change public API, auth, schema, third-party integrations, static-page product code, or unrelated modules without human confirmation.

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
- Every queued Codex Host task gets a task-scoped memory space id. Summaries can be promoted later only through DataMax policy, not by the host process.
- Provider keys and local access keys are never sent to the Codex task prompt.
- Raw stdout/stderr must be redacted and truncated before being shown in runtime inspect.
- The first transport should be a DataMax task queue plus host agent launching `codex exec --output-schema`, not a browser-visible app-server.
- App-server, SDK, and MCP transports must bind only to host-local/private surfaces and remain hidden behind DataMax APIs.
- Codex config is generated from approved DataMax profiles. User text must not set `sandbox_mode`, `approval-policy`, model profile, writable roots, MCP servers, or environment policy.

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

- `exec_schema`: launch `codex exec` with a DataMax-owned output schema and parse only the final JSON response as actionable data.
- `sdk_thread`: use Codex SDK thread control for continuing runs; persist the Codex thread id only as internal worker metadata.
- `app_server`: use local app-server JSON-RPC for richer control; never expose app-server to browser traffic.
- `mcp_server`: use `codex mcp-server` only for controlled framework integration, not as the primary DataMax browser API.

`codex_exec` can launch `codex exec` only after the safety preflight passes. It requires `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=true`, an approved host kind (`windows_jump` or `mac_host`), an execution-capable profile kind, and `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT` pointing at a host-local task workspace root. The returned process output is truncated and redacted before it enters workflow output or AssistantRun events.

Browser traffic still goes only through DataMax APIs. The host agent is a worker attached to the internal workflow queue; it is not a new browser-visible service surface.

## MiniMax Provider Rule

Codex CLI 0.123.0 no longer accepts `wire_api="chat"` providers. MiniMax native `/chat/completions` cannot be configured directly as a Codex provider, and `https://api.minimaxi.com/v1/responses` returned 404 in jump-host validation.

For MiniMax-backed Codex execution, use:

```text
Codex CLI -> private local Responses-compatible shim -> MiniMax /chat/completions
```

The shim must be local/private, must not expose provider keys to browser traffic, and must normalize leading MiniMax `<think>...</think>` blocks before streaming text back to Codex.

## Provider Shim Rule

Any future DataMax-owned Responses-compatible shim must obey the same bridge boundary:

```text
Codex CLI/SDK/app-server -> private provider shim -> upstream provider API
DataMax API/workers          -> AssistantRun, datasets, memory, workflows, artifacts, audit
```

The shim may expose redacted diagnostics to DataMax:

- health and provider status
- profile/capability snapshot
- usage summary and recent usage events
- context-budget and tool-output budget reports
- liveness/protocol-repair events

The shared DataMax contract for this exists as `ProviderShimObservabilitySnapshotView` in `crates/contracts`. `llm-gateway` can already derive the safe profile portion from `ModelProviderProfile`; future shim/host endpoints should fill runtime health, usage, balance, debug trace, budget, and liveness fields from bounded/redacted local diagnostics. AssistantRun Codex packages also carry a tool-output budget policy so oversized execution payloads can be bounded without dropping retrieval evidence, refs, media timestamps, or failure details.

The shim must not expose:

- provider keys
- raw prompts containing user secrets
- raw tool outputs beyond bounded/redacted excerpts
- direct DataMax database access
- direct queue submission
- direct dataset/file access
- browser-visible endpoints
