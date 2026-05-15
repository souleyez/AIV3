# AI Data Platform V3

Rust-first rebuild of the AI Data Platform. The project is now past the initial
skeleton stage: the control plane, workflow/task runtime, several workers,
model-facing inspect surfaces, report host actions, and a first web assistant
shell are all present.

## Current Status

Implemented baseline:

- Rust workspace with explicit crate boundaries for domain, contracts, storage,
  workflow engine, workers, report runtime, gateways, tool registry, and API.
- PostgreSQL-backed system of record for datasets, documents, workflows, tasks,
  artifacts, runtime traces, report plans, rendered outputs, and published
  report versions.
- Axum `platform-api` with dataset, document, chat session, dataset output,
  report plan/render/publish/read, workflow retry, runtime inspect, and tool
  registry surfaces.
- AssistantRun ordinary chat now has a feature-gated Host-Controlled ReAct loop
  with typed action parsing, weak planning catalog, scoped retrieval, bounded
  document-detail reads, report handoff, redacted trace summaries, safe progress
  UI, and optional OpenClaw bridge stubs.
- Worker skeletons and vertical slices for ingest, retrieval, memory directory,
  dataset output, chat session, report planning, and report rendering.
- Model-facing runtime summaries through `runtime.inspect`, including
  capability class, service lane, evidence state, continuation state, and
  recommended tool keys.
- First V3 web assistant shell in `apps/web`, reusing the old assistant layout
  direction while consuming V3 host surfaces directly.

Important current limits:

- Chat now supports both new sessions and appended turns from the web UI, but
  worker completion is still polling-based rather than product-grade streaming.
- Retrieval now uses local lexical signatures, prompt-ranked evidence binding,
  real `retrieval.search` host-tool traces in dataset/chat worker generation,
  and document detail / compare model-facing evidence states, but external
  embedding/vector index integration is still a later phase.
- Streaming/provider/tool-loop runtime contracts are partially modeled but not
  yet product-grade.
- OpenClaw bridge actions are intentionally gated and stubbed; real sidecar
  memory/execution adapters still need a dedicated integration pass.
- The web report center can drive continue/render/publish host actions and show
  plan/detail/version state, but richer artifact preview and failure-specific UI
  affordances are still later polish.

## Repository Layout

- `apps/web` - Next.js assistant UI, default dev port `3100`
- `apps/docs-site` - placeholder docs app
- `crates/platform-api` - Axum HTTP API and local CLI host surfaces
- `crates/storage` - PostgreSQL repositories and migrations
- `crates/workflow-engine` - transition engine
- `crates/workflow-definitions` - workflow catalog
- `crates/*-worker` - task consumers for ingest, retrieval, memory, chat,
  dataset output, and report runtime slices
- `crates/contracts` - API and typed view contracts
- `crates/tool-registry` - model/host tool surface registry
- `docs` - architecture notes, ADRs, execution plans, and validation docs
- `infra/compose` - local dependency stack

## Local Dependencies

The local compose stack includes PostgreSQL, Redis, NATS JetStream, Qdrant, and
MinIO. Fresh environments target PostgreSQL 17.9:

```powershell
docker compose -f .\infra\compose\docker-compose.local.yml up -d
```

PostgreSQL defaults are:

```text
postgres://ai_platform:ai_platform@localhost:5432/ai_data_platform_v3
```

`platform-api` will use that local database by default through the storage
crate's local default.

## Runtime Flags

AssistantRun ReAct is disabled by default and can be enabled per environment:

```powershell
$env:ASSISTANT_RUN_REACT_ENABLED = "true"
$env:ASSISTANT_RUN_REACT_MAX_STEPS = "3"
$env:ASSISTANT_RUN_RUNTIME_PROVIDER = "openclaw" # optional
$env:ASSISTANT_RUN_RUNTIME_MODEL = "assistant-run-react-v1"
```

OpenClaw remains optional and off unless explicitly enabled:

```powershell
$env:OPENCLAW_EXTENSION_ENABLED = "true"
$env:OPENCLAW_GATEWAY_BASE_URL = "http://127.0.0.1:8787"
$env:OPENCLAW_GATEWAY_TOKEN = "<server-side-token>"
$env:OPENCLAW_MEMORY_ENABLED = "true"
$env:OPENCLAW_READONLY_EXECUTION_ENABLED = "true"
$env:OPENCLAW_READONLY_EXECUTION_ALLOWLIST = "inspect_local_index"
```

## Development

Install web dependencies:

```powershell
pnpm install
```

Run the Rust API:

```powershell
$env:PLATFORM_API_ADDR = "127.0.0.1:3000"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo run -p platform-api --bin platform-api"
```

Run the web app:

```powershell
pnpm --filter @ai-data-platform-v3/web dev
```

The web app defaults to `http://127.0.0.1:3100` and proxies `/api/v3/*` to
`platform-api /v1/*`. Override the backend target with:

```powershell
$env:PLATFORM_API_BASE_URL = "http://127.0.0.1:3000"
```

Run common workers from WSL:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo run -p chat-session-worker"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo run -p dataset-output-worker"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo run -p report-planner-worker"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo run -p report-render-worker"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo run -p assistant-run-worker"
```

`assistant-run-worker` consumes the `assistant_run / consume_model_completion_turn`
queue for background model-authored completion turns, such as video/PPT extraction
finishing after the original chat request has already returned.

## Verification

Rust checks:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check --workspace"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test"
```

Web build:

```powershell
pnpm --filter @ai-data-platform-v3/web build
```

Deployment-target smoke for the AssistantRun background completion worker:

```bash
bash scripts/run-assistant-run-worker-smoke.sh
```

## Key Host Surfaces

Representative HTTP surfaces:

- `GET /v1/datasets`
- `POST /v1/datasets`
- `POST /v1/datasets/{dataset_id}/chat-sessions`
- `GET /v1/chat-sessions/{session_id}/messages`
- `POST /v1/chat-sessions/{session_id}/report-entry`
- `POST /v1/datasets/{dataset_id}/outputs`
- `GET /v1/report-plans`
- `POST /v1/report-plans/{plan_id}/continue`
- `POST /v1/report-plans/{plan_id}/renders`
- `POST /v1/report-plans/{plan_id}/publish`
- `GET /v1/report-plans/{plan_id}/published-report`
- `GET /v1/workflow-executions/{execution_id}/runtime-inspect`
- `POST /v1/workflow-executions/{execution_id}/retry`
- `GET /v1/tools`

Representative CLI surfaces live under `crates/platform-api/src/bin`, including:

- `runtime-inspect-cli`
- `chat-session-report-entry-cli`
- `report-render-cli`
- `report-publish-cli`
- `report-read-published-cli`
- `workflow-retry-cli`
- `document-detail-cli`
- `document-compare-cli`
- `retrieval-search-cli`
- `memory-directory-refresh-cli`

## Development Plan

Immediate sequence:

1. Stabilize repository hygiene, dependency versions, README, and verification
   baseline.
2. Keep the web Report Service loop usable while adding richer artifact and
   failure affordances.
3. Keep existing-session chat turn append support hardened across API, worker,
   contracts, and web UI.
4. Turn the responsive web shell into a mobile-usable interaction model.
5. Tighten provider, streaming, tool-loop, artifact commit, and recovery
   semantics.
6. Continue from local lexical retrieval toward external embedding/vector recall
   while keeping worker tool traces and evidence state derivation durable.
7. Freeze the V3 model-facing capability contract after the runtime facts are
   stable enough.

See `docs/plans/2026-04-23-v3-development-plan.md` for the working plan.

## Architecture References

- `docs/architecture/v3-rust-architecture.md`
- `docs/architecture/v3-repository-and-module-layout.md`
- `docs/adr/README.md`
- `docs/validation/runtime-inspect-pretty.md`
