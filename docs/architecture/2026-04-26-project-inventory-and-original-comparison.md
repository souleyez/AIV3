# Project Inventory And Original Comparison

Date: 2026-04-26

## Scope

This note inventories the current DataMax project and compares it with the original `ai-data-platform` project, focused on the static page visual workbench and image-generation path.

Repos inspected:

- Current DataMax: `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3`
- Original app: `C:\Users\soulzyn\Desktop\codex\ai-data-platform`
- Orchestrator dependency: `C:\Users\soulzyn\Desktop\codex-web`
- Cloudflare Codex dependency: `C:\Users\soulzyn\Desktop\codex\cf-codex-workstation`
- Optional front door: `C:\Users\soulzyn\Desktop\codex\cf-codex-frontdoor`

## Executive Summary

DataMax is the stronger long-term platform foundation. It has Rust crate boundaries, durable workflow/runtime records, report plan/render/publish surfaces, retrieval search traces, model-facing runtime summaries, and a first web shell. It is designed to make report planning, rendering, runtime inspection, and recovery explicit.

The original `ai-data-platform` has more static page workbench code, but that workbench should not be treated as the DataMax target UI. It already has a `/static-pages` UI, Fastify routes under `/reports/static-page/*`, deterministic objective drafting, visual brief generation, and a `codex-web` orchestrator client that expects image artifacts. However, the product direction is to redesign this area later instead of cloning or gradually replacing the original workbench.

The current blocker is not the queue key. The 8-server orchestrator key is valid and can submit tasks. The blocker is execution capability: the runner currently completes image tasks without returning an image artifact or `imageBase64`. Direct Cloudflare image endpoint `/api/image/static-page-draft` is also not implemented in `cf-codex-workstation` yet.

## Product Direction Update

As of 2026-04-26, the original static page workbench is not a migration target:

- Do not spend effort making the DataMax frontend match the original static page workbench one-to-one.
- Do not plan a gradual replacement of the original static page workbench.
- Treat the original implementation as reference material only: useful for lessons, API shape examples, and parts that may be extracted later.
- Static page workbench work is not urgent because it has not been formally delivered to customers.
- When this area is resumed, it should be redesigned around the DataMax platform model instead of ported from the original UI.

## Current DataMax State

Repository state:

- Branch: `main`
- Remote state: local branch is ahead of `origin/main` by 2 commits
- Recent local commits:
  - `e12ae4c Add static page visual workbench plan`
  - `2c31894 Route visual image generation through Cloudflare Codex`
- Working tree state changes during this follow-up session; use `git status`
  for the latest dirty-worktree view before committing.

Implemented platform baseline:

- Rust workspace with separated crates for domain, contracts, storage, workflow engine, workflow definitions, platform API, workers, report runtime, report compiler/publisher, retrieval, gateways, and tool registry.
- PostgreSQL-backed state for datasets, documents, workflows, runtime traces, report plans, render outputs, and published report versions.
- Axum `platform-api` with dataset/document/chat/report/workflow/runtime inspect/tool registry surfaces.
- Workers for ingest, retrieval, memory, dataset output, chat sessions, report planning, and report rendering.
- Durable `retrieval.search` host-tool traces in dataset and chat worker flows.
- Model-facing runtime summaries through `runtime.inspect`.
- Web shell that can drive report plan continuation, render, publish, and read surfaces.

Deferred or not implemented yet in DataMax:

- `ReportVisualDraft` domain object/table/repository.
- `static-page-visual-runtime` crate.
- `static-page-visual-worker` crate.
- `POST /v1/static-pages/context`.
- `POST /v1/report-plans/{plan_id}/visual-drafts`.
- `/static-pages` DataMax web workbench.
- Direct DataMax provider client for `cloudflare-codex`.

The DataMax static page visual workbench currently exists as a detailed implementation plan at:

```text
C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3\docs\plans\2026-04-25-static-page-visual-workbench-DataMax-plan.md
```

## Original Project State

Repository state:

- Branch: `codex/dataset-latest-doc-details`
- Head: `5e2ac53 Add static page visual workbench`
- Only observed local dirt: untracked `outputs/`.

Implemented static page workbench pieces:

- Frontend route: `apps/web/app/static-pages/page.js`
- UI component: `apps/web/app/components/StaticPageWorkbench.js`
- Navigation entries for static page workbench.
- Static page API calls from `apps/web/app/home-api.js`.
- Backend routes in `apps/api/src/routes/reports.ts`:
  - `POST /reports/static-page/context`
  - `POST /reports/static-page/visual-draft`
  - `GET /reports/static-page/visual-draft/:taskId`
- Deterministic objective builder:
  - `apps/api/src/lib/static-page-objective-draft.ts`
- Visual brief builder:
  - `apps/api/src/lib/static-page-visual-brief.ts`
- Orchestrator client:
  - `apps/api/src/lib/codex-orchestrator-client.ts`

Original architecture:

- TypeScript/Fastify backend.
- Next.js/React frontend.
- Report center state and draft-module pipeline are the application-local source of truth.
- Visual draft is attached to existing report output flow rather than modeled as a first-class durable workflow object.
- Primary image path is `codex-web` orchestrator; fallback paths were planned for Cloudflare direct and OpenAI direct.

## Key Architecture Difference

| Area | Original `ai-data-platform` | Current DataMax direction |
|---|---|---|
| Backend | Fastify + TypeScript app backend | Axum + Rust platform API |
| State model | Report center state / `ReportOutputRecord.visualDraft` | Dedicated `ReportVisualDraft` domain object and table planned |
| Workflow | App-local task flow and polling | Durable workflow executions, task records, runtime inspect |
| Workbench UI | Already implemented under `/static-pages` | Planned, not implemented |
| Objective drafting | Implemented deterministic local builder | Planned as Rust deterministic builder |
| Visual brief | Implemented TS prompt builder | Planned as Rust brief builder |
| Image provider | `codex-web` orchestrator first; direct fallbacks planned | Cloudflare remote Codex boundary preferred; mock first |
| Source of truth | Dataset/report plan/draft output | Dataset evidence, `ReportPlan`, AST versions, render manifests |
| Publish safety | Visual draft attached to report output state | Visual draft explicitly non-publishable |
| Inspectability | App routes and logs | Model-facing `runtime.inspect` and durable runtime traces |

## Codex-Web Orchestrator State

`codex-web` is not just a draft contract anymore. It now has:

- Task queue and runtime target enforcement.
- Machine-scoped access keys.
- Optional project workspace routing.
- Artifact extraction from final Codex messages.
- Local image artifact storage.
- `includeArtifactData=1` support for inline base64 on polling.
- Guardrails for Cloudflare task usage.

Important current behavior:

- Image tasks are recognized by `kind=image-draft`, `kind=static-page-visual`, or `metadata.output=image-artifact`.
- If final text contains JSON image specs, data URLs, or base64 image artifacts, `codex-web` imports them into artifact storage.
- If an image was required but missing, the completed task gets `artifactStatus: "missing_required"`.

This means the orchestrator contract is ready to carry an image result. The missing piece is the Codex runner actually producing an image artifact.

## Cloudflare Codex State

`cf-codex-workstation` currently exposes:

- `/api/status`
- `/api/start`
- `/api/auth/import`
- `/api/checkpoint`
- `/api/stop`
- `/api/proxy/*`
- `/bridge/*`
- `/responses`

It does not currently implement:

```text
POST /api/image/static-page-draft
```

Observed direct calls:

- `GET https://cf-codex-workstation.soulzyn.workers.dev/api/status` returns `provider_ready: false`.
- `POST https://cf-codex-workstation.soulzyn.workers.dev/api/image/static-page-draft` with bearer token returns `404 not_found`.
- `POST https://codex.souleye.cc/api/image/static-page-draft` returns `405 method_not_allowed`.

So the direct Cloudflare image endpoint described in both plans is still a planned endpoint, not a live implementation.

## 8-Server Queue Key Check

The 8-server env file exists and is usable:

```text
/etc/codex-orchestrator/8-rust-ai-assistant.env
```

Variables present:

- `CODEX_ORCHESTRATOR_BASE_URL`
- `CODEX_ORCHESTRATOR_TASKS_URL`
- `CODEX_ORCHESTRATOR_ACCESS_KEY`
- `CODEX_ORCHESTRATOR_RUNTIME_TARGET_ID`
- `CODEX_ORCHESTRATOR_CLIENT_NAME`
- `CODEX_ORCHESTRATOR_DEFAULT_KIND`

Smoke results:

- `task_82e6683b-98d8-42a9-a02d-9165267480b6`
  - Submitted successfully.
  - Completed successfully.
  - Returned text only: “我会使用当前的 `imagegen` 技能和图像生成工具来做这张无文字首页英雄图草图。”
  - No image artifact.
- `task_33aff444-4dc5-4db6-ab83-5ed0e1ca7ecc`
  - Submitted successfully.
  - Completed successfully.
  - Result text length was 0.
  - Artifact count was 0.

Conclusion: access key and queue are working. The runner does not currently produce image artifacts for these tasks.

## Practical Interpretation

There are three separate layers that should not be conflated:

1. Queue access: working with the 8-server key.
2. Orchestrator artifact contract: implemented in `codex-web`.
3. Actual image generation execution: not working yet in observed runs.

The current failure is layer 3. A task can be accepted, submitted, run, and marked completed while still failing the product requirement because the completed result has no image.

## Recommendation

Short-term path:

1. Deprioritize static page workbench UI migration.
2. Keep the original `ai-data-platform` static page workbench as reference material, not as a target to clone.
3. If image generation is needed for another product surface, first harden the image execution seam:
   - In `codex-web`, treat text-only completion with `artifactStatus: "missing_required"` as a failed visual result by callers.
   - Preferred execution path: add `/api/image/static-page-draft` or a more generic image endpoint to `cf-codex-workstation`, calling the OpenAI Images API directly from Worker secrets and returning `{ imageBase64, mimeType, provider, model }`.
   - Alternative execution path: update the Codex runner prompt/tool environment so `imagegen` actually runs and writes an image artifact.
4. Resume static page design only when there is a concrete customer-facing workflow and redesigned product direction.

Long-term DataMax path:

1. Continue prioritizing the DataMax core platform: dataset workflows, report planning/render/publish, runtime inspection, retrieval traces, and operational reliability.
2. Keep visual generation as an isolated provider capability until there is a redesigned static page product surface.
3. Connect DataMax to Cloudflare only after the Cloudflare endpoint or orchestrator artifact path has passed a real smoke test.
4. Keep direct OpenAI keys out of browser and DataMax platform services; place provider credentials only in Cloudflare Worker secrets or server-side orchestrator runtime.

## Decision Point

The next implementation should not be a DataMax static page UI clone. If image generation becomes necessary for an active surface, start at the image execution seam, because any UI would otherwise only surface the same failure:

```text
accepted task -> completed task -> no image artifact
```

The cleanest image-related task remains:

```text
Implement and deploy cf-codex-workstation POST /api/image/static-page-draft with tests.
```

After that endpoint returns a real PNG/base64 payload, DataMax can safely implement a `cloudflare-codex` provider. Static page UI/workbench work should wait for a fresh product redesign.
