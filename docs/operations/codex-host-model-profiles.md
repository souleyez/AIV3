# Codex Host Model Profiles

**Scope:** DataMax Rust assistant Codex Host integration.

Codex Host model selection is a DataMax policy decision, not a free-form user prompt field. Users can ask for capability and quality, but DataMax maps that request to an approved profile.

## Upstream Codex OSS Baseline

Use upstream `openai/codex` as the execution-kernel baseline:

- Repository: `https://github.com/openai/codex`
- License: Apache-2.0.
- Current observed release on 2026-05-10: `0.130.0`.
- Supported operational surfaces to prefer: `codex exec` for non-interactive work, `--output-schema` for structured final JSON, `@openai/codex-sdk` for server-side TypeScript thread control, app-server JSON-RPC for richer local control, and `codex mcp-server` for tool-style integration.

Do not fork Codex into DataMax unless a specific upstream gap blocks these surfaces. Treat a fork as a last-resort patch set, not the main integration path.

## CoDeepSeedeX Reference Pattern

`CoDeepSeedeX` is a useful reference for provider adaptation, not a runtime dependency for DataMax.

- Repository: `https://github.com/Awenforever/CoDeepSeedeX/tree/master`
- License: MIT.
- Shape: local OpenAI Responses-compatible proxy that lets Codex use DeepSeek-backed profiles.
- Valuable pattern: create Codex profiles that point at local-only provider shims, with separate stable/thinking profiles when the upstream provider exposes different reasoning modes.
- Valuable operations surface: `/healthz`, provider status, balance when available, usage summary/events, debug trace status, and context-budget diagnostics.
- Valuable hardening surface: context trimming, semantic/persistent compaction experiments, tool-output budget reports, protocol repair for tool calls, and liveness recovery when a model stalls mid-tool loop.

DataMax should borrow these ideas into its own model-gateway/Codex profile system:

- Implement DataMax-owned shims only where Codex cannot call a provider natively.
- Keep shims bound to local/private interfaces such as `127.0.0.1`.
- Keep provider keys outside task prompts, browser APIs, user-visible logs, and artifact payloads.
- Persist product-grade usage/audit in PostgreSQL/runtime inspect. Shim-local SQLite or JSONL files are host diagnostics only.
- Treat debug traces as sensitive because they can contain request summaries, paths, tool-output summaries, and usage details.
- Do not let the provider shim execute DataMax tools, access datasets, own memory, write queues, or bypass DataMax action validation.

## Profile Rules

- Profiles are server-side config only.
- User text cannot pass raw Codex CLI flags.
- Provider keys never enter task prompts or user-visible logs.
- Runtime events record profile id, provider family, model label, and capability class.
- Real execution profiles stay disabled until tested on `windows-jump` or the later Mac host.
- The local developer workstation must not be used for Codex execution tests.
- Task prompts must not include provider keys, browser-local keys, or raw secrets.
- Each host task runs under a DataMax-created `task_memory_space_id`; the host can return summaries, but DataMax decides whether anything is promoted into conversation or project memory.
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
allowed_capabilities = ["assistant_conversation", "static_page_plan", "static_page_edit", "static_page_advanced_publish"]

[profiles.cloudflare-codex-fixed-tasks]
kind = "codex-native"
enabled = false
transport = "exec_schema"
model = "gpt-5.3-codex"
allowed_capabilities = ["static_page_image2_data_publish", "answer_quality_autofix", "data_ingestion_analysis"]

[profiles.rightcode-gpt-5-5-high]
kind = "codex-compatible-shim"
enabled = true
transport = "exec_schema"
provider = "rightcode"
model = "gpt-5.5"
wire_api = "responses"
base_url = "<server-configured-rightcode-codex-url>"
env_key = "RIGHTCODE_API_KEY_MAIN"
allowed_capabilities = ["customer_complex_request", "customer_artifact_request", "generated_static_page_edit", "generated_static_page_publish", "static_page_image2_data_publish", "data_ingestion_analysis"]

`rightcode-gpt-5-5-high` is the intended profile for the 8-server customer-facing Web Codex executor when deployment is explicitly approved. Treat it as a general customer Codex lane, not a static-page-only lane: customer analysis, planning, data/report work, document/package/script creation, and customer artifact revision can be routed to it after DataMax preflight. The profile still must not allow customer traffic to modify V3 product source, services, migrations, auth, public APIs, provider configuration, deploy state, commits, system files, or `/srv/aiv3/repo`.

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
exec_schema -> codex exec with a DataMax-owned output schema for one-shot worker tasks
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
assistant chat / static-page action -> DataMax AssistantRun context package
  -> profile transport=exec_schema or sdk_thread
  -> Codex receives only DataMax-supplied context/tool contracts
  -> Codex returns structured response/action intent
  -> DataMax validates and executes requested action
```

`codex_exec` must require all of these before it can be wired:

```text
CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=true
CODEX_HOST_AGENT_HOST_KIND=windows_jump|mac_host|linux_host|aiv3_server
CODEX_HOST_AGENT_PROFILE_KIND=codex-native|codex-compatible-shim
CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT=<host-local task workspace root>
profile capability allowlist contains the requested capability
task context includes a DataMax-created assistant_run_id and task_memory_space_id
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

Customer Web Codex result policy:

```text
customer_complex_request final output should include customer_result_summary JSON
customer_artifact_request and generated_static_page_* may also include customer_result_summary after writing manifests
customer_result_summary schema is v3.customer_codex_result_summary
codex_exec writes schemas/customer-result-summary.schema.json into the task workspace
codex_exec passes --output-schema schemas/customer-result-summary.schema.json by default for customer Web Codex capabilities
CODEX_HOST_AGENT_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED=false disables the flag only for older Codex CLI compatibility
browser-facing fields are bounded title, summary, findings, recommended_next_actions, warnings, artifact_intent, capability, and safety flags
host-agent must rebuild the summary from structured JSON and drop raw stdout/stderr, raw prompts, provider logs, credentials, absolute paths, database URLs, cookies, and token-like strings
web UI renders the summary on the Codex task card; file bundles still come from validated customer artifact manifests
```

Retry, timeout, cancellation, and workspace-retention defaults:

```text
short planning tasks: 2-5 minutes
static-page publish: 20-30 minutes
data-ingestion analysis: 10-30 minutes depending on sample size
default CODEX_HOST_AGENT_TASK_TIMEOUT_MS=1800000
default CODEX_HOST_AGENT_TASK_WORKSPACE_RETENTION_HOURS=168
max attempts: 3
retry: transient execution or Cloudflare poll timeout only
needs_human: never auto-retry
cancelled: emit codex_host_task.cancelled with retryable=false and no stdout/stderr/prompt
task workspace retention: write retention_policy into runtime.json; cleanup is an explicit operator job
```

Each fixed-task workspace includes `runtime.json.retention_policy` with the configured retention window, `cleanup_requires_operator=true`, and `backup_before_delete=true`. Locally initiated cleanup must use the workspace backup-first deletion helper. Server cleanup should be a separate deployment operation with an operator-visible retention window and audit note; it must not run as part of normal third-party polling.

Write-capable Codex Host tasks need a separate capability and a stronger approval policy:

```text
capability=propose_patch
workspace=isolated branch or copied worktree
write_access=true
commit_access=false by default
human_review_required=true
```

Customer-facing Codex executor routing uses narrower capabilities than `propose_patch`:

```text
capability=customer_complex_request
workspace=task workspace
write_access=false
purpose=analyze complex customer requests and return a structured answer/action intent

capability=customer_artifact_request
workspace=isolated customer task workspace
write_access=task workspace only
purpose=let customers use Codex from the web UI to create or revise customer-facing artifacts without touching V3 product code
output_manifest=customer-artifact-manifest.json with workspace-relative artifact paths
publish_access=host may copy validated manifest files to /generated-artifacts/customer-codex/... when CODEX_HOST_AGENT_CUSTOMER_ARTIFACT_PUBLISH_ENABLED is true

capability=generated_static_page_edit
workspace=task workspace seeded with a V3-generated static page artifact
write_access=task workspace only
publish_access=host may copy validated workspace files to generated-artifacts, but DataMax still validates static-page semantics before treating the page as a durable template/version
output_manifest=customer-artifact-manifest.json or generated-artifacts/manifest.json
workspace_seed=workspace-seed.json plus existing-artifact/ materialized from the current generated static page when available

capability=generated_static_page_publish
workspace=isolated task workspace scoped by DataMax selected-scope/evidence context; optional DataMax static-page package seed
write_access=task workspace only
publish_access=host may copy validated workspace files to generated-artifacts, but overwrite/stable URL/version promotion still requires DataMax policy validation
```

Customer traffic must not use `propose_patch`. Requests to change V3 product behavior, source code, services, migrations, auth, public API, provider configuration, or deployment state are `v3_product_change_request` and must return an operator-review outcome instead of a writable Codex task. Platform API should record `assistant_run.codex_sidecar_scope_blocked` with `status=needs_operator_review` and keep the normal answer path available; the model-facing prompt must not promise that product code, services, or deployment were changed. General customer work can use `customer_complex_request` for read-only analysis or `customer_artifact_request` for isolated customer artifact creation. V3-generated static pages are a specialized customer artifact and may be edited through `generated_static_page_edit` or `generated_static_page_publish` after DataMax scopes the workspace and validates the output. Writable customer tasks must write a workspace-local manifest; host output exposes only safe relative paths, artifact metadata, size, sha256, and generated-artifact public URLs produced by the host copy step. Manifest output must not reference reserved workspace inputs such as `existing-artifact/`, `workspace-seed.json`, `task.json`, `runtime.json`, `schemas/`, or `evidence/`. Platform API may project the host ready event into `codex_customer_artifact_bundle` output artifacts for the web UI. Workspace-only bundles remain `published=false`; host-published bundles may expose `published=true`, `primary_url`, `published_manifest_url`, and per-file `public_url` only when those URLs pass the `/generated-artifacts/` allowlist.

`generated_static_page_publish` uses the generic `assistant_run.customer_artifact_request_artifacts_ready` lifecycle event name, but the payload must still carry `capability=generated_static_page_publish` and `route=generated_static_page_publish`. Platform API and the web normalizer must preserve that route while rejecting arbitrary or unsafe routes. Platform API projected artifact bundles are allowlisted only to artifact-producing capabilities/routes: `customer_artifact_request`, `generated_static_page_edit`, and `generated_static_page_publish`; non-artifact capabilities such as `customer_complex_request` and `v3_product_change_request` must not become customer artifact bundles. The web task-card normalizer may still show the broader customer Codex status set: `customer_complex_request`, `customer_artifact_request`, `generated_static_page_edit`, `generated_static_page_publish`, and `v3_product_change_request`. Browser-visible and runtime-inspect-visible text such as artifact title/summary/file title, task failure reason, and permission scope must be dropped or replaced with a safe default if it resembles credentials, tokens, database URLs, raw logs/prompts, absolute local paths, `/srv/aiv3/repo`, `/srv/aiv3/shared`, `.env`, or other internal paths.

Dataset-bound static-page/report templates are a governed customer artifact case. If an accepted template baseline exists for the dataset/default-prompt scope and the customer did not explicitly ask for a redesign or new effect image, reuse the accepted baseline by default. View/delivery requests return the accepted link. Normal create/refresh requests expose `reuse_reason=default_dataset_template_reuse` and must not create a new random visualization surface. Concrete edit requests use the existing template/page as the visual contract and route to the controlled Codex update path. Only explicit redesign/new-style/from-scratch requests may create a new candidate effect-image flow.

Advanced static-page publishing should start as a read-only/plan-only capability:

```text
capability=static_page_advanced_publish
purpose=inspect static-page requirements, identify data口径 risks, propose real-data HTML/artifact edits, and summarize publish steps
default_mode=plan_only
write_access=false until isolated workspace and human confirmation are enabled
publish_access=false until DataMax validates artifact output and records audit
```

Cloudflare Codex fixed templates are a narrower execution profile, not a free-form prompt profile:

```text
profile=cloudflare-codex-fixed-tasks
transport=exec_schema
allowed_templates=static_page_image2_data_publish, answer_quality_autofix, data_ingestion_analysis
task_source=server_owned_template_package_only
user_prompt_cli_flags_allowed=false
```

`static_page_image2_data_publish` may publish a new generated artifact without per-task human confirmation only when DataMax validates `publish_mode=new_generated_artifact_only`, artifact path is under `/generated-artifacts/`, and the output contains a snapshot/date/unit validation report. Overwrite, stable URL replacement, source-code changes, credential/scope expansion, and uncertain口径 still require human confirmation.

`answer_quality_autofix` may diagnose and propose low-risk answer-quality patches without per-task confirmation only inside the fixed answer-quality allowlist. It must not deploy automatically and must not touch public API, auth, schema, static-page product code, third-party contracts, or unrelated files.

`data_ingestion_analysis` may run without per-task confirmation only as read-only profiling or a staging/import-spec proposal over DataMax-selected sources. It must not request or emit credentials/database URLs, write production tables, migrate schema, change public API/auth/third-party fields, or expand source permissions. Any such request returns `needs_human`.

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
- DataMax exposes a private, local-only Codex-compatible provider shim.
- Codex task uses DataMax tools that call `llm-gateway`, while Codex itself keeps its native model.

The first production path should prefer DataMax-owned `llm-gateway` for MiniMax and keep Codex Host focused on execution.

If MiniMax must be used as the Codex model itself, the profile must target a private Responses-compatible shim. Do not configure MiniMax Chat Completions directly as a Codex provider until upstream Codex supports that provider shape.

## Provider-Shim Observability Contract

Every DataMax-owned Codex-compatible provider shim should expose enough diagnostics for the platform to decide whether a profile is healthy before routing a conversation through it:

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

These diagnostics must be redacted, bounded, and linked to DataMax `AssistantRun` or workflow ids when possible. They are for operations and quality control; they are not user-facing answer content.

Current DataMax-side foundation:

- `contracts` defines a Provider Shim observability snapshot with health, profile/capability snapshot, usage summary/events, optional balance, debug-trace status, context-budget report, tool-output budget, and liveness events.
- `llm-gateway` can build a safe profile/status snapshot from `ModelProviderProfile` without exposing raw provider keys or raw base URLs.
- `llm-gateway` can also convert provider runtime metadata into redacted Provider Shim usage events and summaries, ready for DataMax runtime inspect or future PostgreSQL audit persistence.
- AssistantRun detail responses now include safe diagnostics for the latest Codex shadow comparison, context-budget pressure, jump-host/Mac-host validation readiness, completed Codex Host validation output summaries with `host_kind`, and redacted provider usage events. DataMax treats completed host validation as valid only when it comes from `windows_jump` or `mac_host`, giving audit/runtime views a stable read path without scanning raw event payloads in the browser.
- Real Codex conversation transports are now triple-gated in practice: without `ASSISTANT_RUN_CODEX_REAL_TRANSPORT_FEATURE_GATE=enabled`, DataMax records the requested transport but runs the effective Codex executor in `codex_plan_only`; even with that feature gate enabled, DataMax still requires `ASSISTANT_RUN_CODEX_REAL_TRANSPORT_PROMOTION_REVIEW_APPROVED=approved` before the requested real transport can reach `assistant-runtime`. Current runtime behavior still returns an unsupported-transport blueprint with `codex_invoked=false` until host validation and real transport wiring are implemented. The separate promotion-review gate is intentionally manual and should only be enabled after the `promotion_gate` diagnostics show stable shadow comparison plus valid jump-host/Mac-host smoke output.
- Promotion review also depends on the read-only `codex_executor.model_gateway_gate`. This gate must report `status=ready` before `promotion_gate.status` can become `eligible_for_feature_gate_review`; otherwise DataMax blocks with `blocked_by_model_gateway`. The gate checks that the Codex conversation model profile is present, auth is configured, and the wire/capability surface supports either Codex-compatible execution or JSON action output. Typical blocking statuses are `profile_missing`, `auth_not_configured`, and `unsupported_codex_surface`.
- AssistantRun Codex context packages now carry a tool-output budget policy; DataMax only trims oversized `tool_outputs` payload fields and preserves evidence refs, source locators, media timestamps, and error/status details.
- AssistantRun Codex diagnostics can now summarize a supplied provider-shim observability snapshot through a fixed safe field set: health status, profile/model/wire API, usage counts, context-budget pressure, tool-output trimming counts, and liveness event status. It intentionally does not expose auth env names, raw provider errors, raw request ids, trace ids, raw balance amounts, debug notes, or raw request/response payloads.
- For `codex_compatible_shim` profiles, DataMax also synthesizes a conservative provider-shim observability snapshot inside Codex diagnostic event payloads and execution trail entries before a real shim process reports health. This gives runtime inspect a stable field shape while keeping health `unknown`, usage zeroed, and profile/auth details redacted.
- The snapshot remains a contract for local shim/host diagnostics and does not grant the shim access to DataMax tools, datasets, queues, or memory.

Before enabling any real Codex conversation transport, confirm the three diagnostics together:

```text
codex_executor.shadow_gate.status            -> eligible_for_host_validation
codex_executor.host_validation_summary.status -> validated
codex_executor.model_gateway_gate.status      -> ready
```

The same states are also exposed in `codex_executor.promotion_gate.readiness_checks` so runtime and observability panels can render the three required checks without inferring readiness from free-form statuses. If any of the three is not ready, keep direct execution authoritative and leave Codex mutation plus queue submission disabled. Do not treat a healthy provider shim alone as approval to enable real transport; it only satisfies the model/profile side of the promotion review.

## Contract Boundary

The worker consumes workflow context generated from `CodexHostTaskRequestView` and records workflow output shaped as `CodexHostTaskOutputView`. These shared contracts live in `crates/contracts` so `codex-host-agent` can stay independent of `platform-api`.

The worker dependency direction should stay narrow:

```text
codex-host-agent -> contracts + workflow-definitions + workflow-engine + storage + event-bus
codex-host-agent -X-> platform-api
browser -X-> codex-host-agent
```

This keeps browser authentication, dataset visibility, report/static-page APIs, and user-facing orchestration inside DataMax while allowing Codex Host to remain an optional execution extension.
