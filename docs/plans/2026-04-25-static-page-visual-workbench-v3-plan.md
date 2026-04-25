# Static Page Visual Workbench V3 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a V3-native static page visual workbench where the first customer-visible artifact is a polished visual draft image, followed by an editable/static report render from the same report plan and dataset evidence.

**Architecture:** Keep `ReportPlan`, report AST versions, retrieval evidence, and report render outputs as the source of truth. Add a new durable `ReportVisualDraft` layer in front of report rendering, with mock generation locally and real GPT image generation delegated to the user's Cloudflare remote Codex endpoint. The visual draft guides layout and style only; it must never become the factual source or a publishable report version by itself.

**Tech Stack:** Rust workspace (`domain-model`, `contracts`, `storage`, `platform-api`, `workflow-definitions`, new visual worker/runtime), PostgreSQL, local filesystem asset storage for mock/local copies, Cloudflare Workers remote Codex endpoint in `C:\Users\soulzyn\Desktop\codex\cf-codex-workstation` with optional frontdoor in `C:\Users\soulzyn\Desktop\codex\cf-codex-frontdoor`, Next.js web shell in `apps/web`, existing `report-planner-worker` and `report-render-worker`.

---

## Positioning

This plan adapts the old `ai-data-platform` static page visual workbench idea to the current V3 architecture.

Do not copy the old Fastify / TypeScript API shape. V3 uses Rust host surfaces, workflow tasks, typed contracts, model-facing summaries, and durable runtime inspect rows.

The first implementation target is a mock-only MVP:

- The workbench can infer or accept a static page objective.
- It can start from an existing planned `report_plan`.
- It can create a durable visual draft record.
- It can render a deterministic mock image preview from local bytes.
- The web UI shows the image before asking the user to render/publish the editable report.

Real image generation is intentionally a Cloudflare-side second step. V3 must not call OpenAI image APIs directly in production; it calls a protected Cloudflare remote Codex image endpoint. Do not hard-code a current OpenAI image model in V3; forward a configurable `model` string to Cloudflare and verify the correct provider API shape against official docs when implementing the Cloudflare worker.

## Non-Negotiable Product Rules

- The image is a retention artifact, not source of truth.
- The source of truth remains dataset evidence, `ReportPlan`, `ReportPlanAstVersion`, and render output manifests.
- Visual drafts are not publishable report versions.
- Mock provider must be shippable and testable without image credits.
- Any real provider must be `cloudflare-codex` behind `STATIC_PAGE_VISUAL_DRAFT_ENABLED=true`.
- Browser code and V3 Rust services must never see the OpenAI image API key.
- Cloudflare stores image-provider secrets and handles provider-specific request/response parsing.
- If visual generation fails, the report plan remains usable.
- If editable report rendering fails after visual success, the image remains visible.

## Proposed V3 Data Shape

Add a new domain object instead of reusing `report_render_outputs`. Reusing render outputs would make visual drafts look publishable because publishing currently checks asset paths on render outputs.

```rust
pub struct ReportVisualDraft {
    pub id: ReportVisualDraftId,
    pub tenant_id: TenantId,
    pub execution_id: WorkflowExecutionId,
    pub plan_id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub ast_version_id: Option<ReportPlanAstVersionId>,
    pub status: ReportVisualDraftStatus,
    pub provider: String,
    pub model: String,
    pub image_object_key: Option<String>,
    pub image_url_path: Option<String>,
    pub remote_request_id: Option<String>,
    pub remote_asset_url: Option<String>,
    pub mime_type: Option<String>,
    pub prompt: String,
    pub revised_prompt: Option<String>,
    pub visual_style: String,
    pub objective: String,
    pub error: Option<String>,
    pub draft_manifest: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum ReportVisualDraftStatus {
    Generating,
    Ready,
    Failed,
}
```

Recommended table:

```sql
create table if not exists report_visual_drafts (
  id uuid primary key,
  tenant_id uuid not null references tenants(id),
  execution_id uuid not null references workflow_executions(id),
  plan_id uuid not null references report_plans(id),
  dataset_id uuid not null references datasets(id),
  ast_version_id uuid references report_plan_ast_versions(id),
  status text not null,
  provider text not null,
  model text not null,
  image_object_key text,
  image_url_path text,
  remote_request_id text,
  remote_asset_url text,
  mime_type text,
  prompt text not null,
  revised_prompt text,
  visual_style text not null,
  objective text not null,
  error text,
  draft_manifest jsonb not null default '{}'::jsonb,
  created_at timestamptz not null,
  updated_at timestamptz not null
);

create index if not exists idx_report_visual_drafts_plan_created
  on report_visual_drafts(tenant_id, plan_id, created_at desc);
```

## Host Surface Sketch

```text
POST /v1/static-pages/context
POST /v1/report-plans/{plan_id}/visual-drafts
GET  /v1/report-plans/{plan_id}/visual-drafts
GET  /v1/static-page-visual-drafts/{draft_id}
GET  /v1/static-page-visual-drafts/{draft_id}/image
```

`POST /v1/static-pages/context` is lightweight and synchronous. It prepares an objective draft and recommends a report plan when possible.

`POST /v1/report-plans/{plan_id}/visual-drafts` creates a workflow execution and task. The worker writes the durable visual draft.

The image endpoint reads local MVP assets from a scoped directory:

```text
storage/static-page-visual-drafts/<draft-id>/draft.png
```

Use object keys in persisted state so this can move to R2/S3/MinIO later without changing contracts.

## Cloudflare Remote Codex Image Boundary

Real GPT image generation belongs in the Cloudflare remote Codex layer, not in the V3 Rust platform.

Existing Cloudflare projects:

- `C:\Users\soulzyn\Desktop\codex\cf-codex-workstation`
- `C:\Users\soulzyn\Desktop\codex\cf-codex-frontdoor`

Preferred public endpoint:

```text
POST https://codex.souleye.cc/api/image/static-page-draft
```

Workstation direct endpoint:

```text
POST https://cf-codex-workstation.soulzyn.workers.dev/api/image/static-page-draft
```

V3 request shape:

```json
{
  "requestId": "report_visual_draft_uuid",
  "prompt": "Chinese enterprise static page visual draft...",
  "model": "configured-by-v3-or-cloudflare",
  "size": "1536x1024",
  "quality": "high",
  "responseFormat": "b64_json",
  "metadata": {
    "source": "ai-data-platform-v3",
    "datasetId": "dataset_uuid",
    "reportPlanId": "report_plan_uuid",
    "visualStyle": "premium_consulting"
  }
}
```

V3 response shape:

```json
{
  "status": "ready",
  "provider": "cloudflare-codex",
  "model": "actual-image-model",
  "imageBase64": "...",
  "mimeType": "image/png",
  "prompt": "...",
  "revisedPrompt": null,
  "remoteRequestId": "cf-image-...",
  "remoteAssetUrl": null,
  "generatedAt": "2026-04-25T00:00:00Z"
}
```

Security rules:

- V3 calls Cloudflare with `Authorization: Bearer <STATIC_PAGE_VISUAL_DRAFT_REMOTE_TOKEN>`.
- Cloudflare stores image provider keys as Worker secrets.
- The browser never calls Cloudflare image generation directly.
- The browser never receives provider credentials.
- Cloudflare should avoid logging full prompts when prompts include customer evidence.
- Cloudflare should log request ids, provider/model, status, latency, and sanitized error kinds.

Cloudflare implementation notes:

- Implement the endpoint in `cf-codex-workstation` first.
- Add optional frontdoor pass-through only after workstation tests pass.
- Keep this endpoint independent of any generic Codex Responses proxy.
- Validate request size and prompt length before calling the image provider.
- Return base64 image bytes for V3 local persistence first.
- Later, Cloudflare may save the image to R2 and return `remoteAssetUrl`; V3 should still persist durable metadata.

## Task 1: Add Visual Draft Domain And Contract Types

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: `crates/domain-model/src/lib.rs`
- Test: `crates/contracts/src/lib.rs`

**Step 1: Write failing domain tests**

Add tests for stable status strings:

```rust
#[test]
fn report_visual_draft_status_roundtrips_through_stable_strings() {
    assert_eq!(ReportVisualDraftStatus::Generating.as_str(), "generating");
    assert_eq!(ReportVisualDraftStatus::from_str("ready"), Some(ReportVisualDraftStatus::Ready));
    assert_eq!(ReportVisualDraftStatus::from_str("failed"), Some(ReportVisualDraftStatus::Failed));
    assert_eq!(ReportVisualDraftStatus::from_str("unknown"), None);
}
```

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p domain-model report_visual_draft_status_roundtrips -- --nocapture"
```

Expected: FAIL because the type does not exist.

**Step 2: Implement minimal domain types**

Add:

- `id_type!(ReportVisualDraftId)`
- `ReportVisualDraftStatus`
- `ReportVisualDraft`

Keep `provider` and `model` as strings. Do not add provider-specific enums yet.

**Step 3: Add contract views**

Add:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportVisualDraftView {
    pub id: ReportVisualDraftId,
    pub execution_id: WorkflowExecutionId,
    pub plan_id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub ast_version_id: Option<ReportPlanAstVersionId>,
    pub status: ReportVisualDraftStatusView,
    pub provider: String,
    pub model: String,
    pub image_url_path: Option<String>,
    pub remote_request_id: Option<String>,
    pub remote_asset_url: Option<String>,
    pub mime_type: Option<String>,
    pub prompt: String,
    pub revised_prompt: Option<String>,
    pub visual_style: String,
    pub objective: String,
    pub error: Option<String>,
    pub draft_manifest: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
}
```

Also add request/response contracts:

- `StaticPageObjectiveDraftView`
- `PrepareStaticPageContextRequest`
- `PrepareStaticPageContextResponse`
- `CreateReportVisualDraftRequest`
- `CreateReportVisualDraftResponse`

**Step 4: Run tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p domain-model && cargo test -p contracts"
```

**Step 5: Commit**

```powershell
git add crates/domain-model/src/lib.rs crates/contracts/src/lib.rs
git commit -m "Add visual draft contract types"
```

## Task 2: Add Storage Migration And Repository

**Files:**

- Modify: `crates/storage/src/lib.rs`
- Test: `crates/storage/src/lib.rs`

**Step 1: Write failing storage tests**

Add tests for:

- migration SQL mentions `report_visual_drafts`
- status helper roundtrips
- repository mapping preserves `image_object_key`, `image_url_path`, `draft_manifest`
- repository mapping preserves `remote_request_id` and `remote_asset_url`

Use existing storage unit-test style; avoid requiring a live database for pure migration checks.

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage report_visual_drafts -- --nocapture"
```

Expected: FAIL.

**Step 2: Add migration SQL**

Add the table and index to the initial schema block, following the existing migration style in `crates/storage/src/lib.rs`.

**Step 3: Add repository input types**

Add:

```rust
pub struct NewReportVisualDraft {
    pub execution_id: WorkflowExecutionId,
    pub plan_id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub ast_version_id: Option<ReportPlanAstVersionId>,
    pub status: ReportVisualDraftStatus,
    pub provider: String,
    pub model: String,
    pub image_object_key: Option<String>,
    pub image_url_path: Option<String>,
    pub remote_request_id: Option<String>,
    pub remote_asset_url: Option<String>,
    pub mime_type: Option<String>,
    pub prompt: String,
    pub revised_prompt: Option<String>,
    pub visual_style: String,
    pub objective: String,
    pub error: Option<String>,
    pub draft_manifest: Value,
    pub created_at: DateTime<Utc>,
}
```

Add update helpers:

- `mark_ready(...)`
- `mark_failed(...)`
- `list_by_plan(...)`
- `get_by_id(...)`

**Step 4: Run targeted and workspace tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage report_visual_drafts && cargo check --workspace"
```

**Step 5: Commit**

```powershell
git add crates/storage/src/lib.rs
git commit -m "Persist report visual drafts"
```

## Task 3: Add Local Visual Draft Asset Store

**Files:**

- Create: `crates/platform-api/src/static_page_visual_assets.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Test that the helper:

- rejects path traversal in ids/file names
- writes PNG bytes under `storage/static-page-visual-drafts/<draft-id>/draft.png`
- returns a stable image URL path `/v1/static-page-visual-drafts/<id>/image`
- can store Cloudflare-returned base64 bytes as a local copy without trusting remote file names

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api visual_draft_asset -- --nocapture"
```

Expected: FAIL.

**Step 2: Implement scoped filesystem helpers**

Use only `ReportVisualDraftId` to build paths. Do not accept caller-provided path fragments.

Suggested API:

```rust
pub fn visual_draft_asset_root() -> PathBuf;
pub fn visual_draft_image_path(root: &Path, draft_id: ReportVisualDraftId) -> PathBuf;
pub fn visual_draft_image_url_path(draft_id: ReportVisualDraftId) -> String;
pub fn write_visual_draft_image(root: &Path, draft_id: ReportVisualDraftId, bytes: &[u8]) -> Result<String>;
```

The local asset store is still needed even when Cloudflare returns the image. V3 stores a local copy for stable `/v1/static-page-visual-drafts/{draft_id}/image` reads; `remote_asset_url` is metadata, not the primary browser image source in MVP.

**Step 3: Run tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api visual_draft_asset"
```

**Step 4: Commit**

```powershell
git add crates/platform-api/src/static_page_visual_assets.rs crates/platform-api/src/lib.rs
git commit -m "Add local visual draft asset storage"
```

## Task 4: Add Objective Draft Builder

**Files:**

- Create: `crates/platform-api/src/static_page_objective.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Test:

- chat/report-entry request produces high-confidence title/objective
- existing `ReportPlan.objective` produces report-plan source
- dataset fallback produces low-confidence objective
- selected dataset/report labels are preserved

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page_objective -- --nocapture"
```

Expected: FAIL.

**Step 2: Implement deterministic builder**

Inputs:

```rust
pub struct StaticPageObjectiveInput {
    pub request_text: Option<String>,
    pub chat_history: Vec<contracts::ChatMessageSummary>,
    pub report_plan: Option<ReportPlan>,
    pub dataset_title: Option<String>,
    pub time_range: Option<String>,
    pub content_focus: Option<String>,
}
```

Output: `StaticPageObjectiveDraftView`.

Prefer deterministic local text first. Do not call an LLM for MVP.

**Step 3: Add route handler for `POST /v1/static-pages/context`**

The route should return only lightweight context:

```json
{
  "objective_draft": {
    "title": "Customer report static page",
    "objective": "Show dataset performance, key risks, and next actions.",
    "source": "report_plan",
    "confidence": "medium",
    "editable": true
  },
  "recommended_report_plan_id": "..."
}
```

**Step 4: Run tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page_objective"
```

**Step 5: Commit**

```powershell
git add crates/platform-api/src/lib.rs crates/platform-api/src/static_page_objective.rs
git commit -m "Add static page objective drafting"
```

## Task 5: Add Visual Brief Builder

**Files:**

- Create: `crates/platform-api/src/static_page_visual_brief.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Use a fake `ReportPlanAstVersion` AST and assert the brief includes:

- objective
- visual style
- module/section titles
- retrieval evidence count when available
- strict instruction not to invent factual values
- strict instruction that the image is not source of truth

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page_visual_brief -- --nocapture"
```

Expected: FAIL.

**Step 2: Implement brief builder**

Suggested structure:

```rust
pub struct StaticPageVisualBriefInput {
    pub plan: ReportPlan,
    pub ast_version: Option<ReportPlanAstVersion>,
    pub objective: String,
    pub visual_style: String,
    pub retrieval_evidence_count: usize,
}

pub struct StaticPageVisualBrief {
    pub prompt: String,
    pub manifest: Value,
}
```

The prompt should be customer-demo oriented:

- premium consulting/report feel
- desktop static page visual
- no browser chrome
- no admin dashboard look
- concise business text blocks
- do not invent numbers

**Step 3: Run tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page_visual_brief"
```

**Step 4: Commit**

```powershell
git add crates/platform-api/src/lib.rs crates/platform-api/src/static_page_visual_brief.rs
git commit -m "Build static page visual briefs"
```

## Task 6: Add Mock Visual Draft Runtime

**Files:**

- Create: `crates/static-page-visual-runtime/Cargo.toml`
- Create: `crates/static-page-visual-runtime/src/lib.rs`
- Modify: `Cargo.toml`
- Test: `crates/static-page-visual-runtime/src/lib.rs`

**Step 1: Write failing provider tests**

Test:

- `mock` provider returns deterministic PNG bytes
- provider result includes provider/model/prompt/generated_at
- disabled config returns a clear unavailable error

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-visual-runtime -- --nocapture"
```

Expected: FAIL because the crate does not exist.

**Step 2: Add provider trait**

```rust
pub struct StaticPageVisualDraftProviderInput {
    pub prompt: String,
    pub visual_style: String,
    pub aspect_ratio: StaticPageAspectRatio,
    pub quality: StaticPageVisualQuality,
}

pub struct StaticPageVisualDraftProviderResult {
    pub provider: String,
    pub model: String,
    pub image_bytes: Vec<u8>,
    pub mime_type: String,
    pub prompt: String,
    pub revised_prompt: Option<String>,
    pub remote_request_id: Option<String>,
    pub remote_asset_url: Option<String>,
    pub generated_at: DateTime<Utc>,
}

pub trait StaticPageVisualDraftProvider: Send + Sync {
    fn generate(&self, input: &StaticPageVisualDraftProviderInput) -> Result<StaticPageVisualDraftProviderResult>;
}
```

**Step 3: Add mock provider**

Embed a tiny deterministic PNG fixture in code or under crate test fixtures. Keep it small.

**Step 4: Add env-based provider selection**

Support:

- `STATIC_PAGE_VISUAL_DRAFT_ENABLED`
- `STATIC_PAGE_VISUAL_DRAFT_PROVIDER=mock`
- `STATIC_PAGE_VISUAL_IMAGE_MODEL`

Real provider names can be accepted later but should return unavailable until implemented.

**Step 5: Run tests and check**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-visual-runtime && cargo check --workspace"
```

**Step 6: Commit**

```powershell
git add Cargo.toml crates/static-page-visual-runtime
git commit -m "Add mock static page visual runtime"
```

## Task 6A: Add Cloudflare Remote Codex Image Endpoint

**Files:**

- Modify: `C:\Users\soulzyn\Desktop\codex\cf-codex-workstation\src\index.ts`
- Modify/Add test: `C:\Users\soulzyn\Desktop\codex\cf-codex-workstation\test\index.spec.ts`
- Optional Modify: `C:\Users\soulzyn\Desktop\codex\cf-codex-frontdoor\worker.js`
- Optional Modify: `C:\Users\soulzyn\Desktop\codex\cf-codex-workstation\README.md`

**Step 1: Write failing auth tests**

Add tests:

- `POST /api/image/static-page-draft` without bearer token returns `401`
- wrong bearer token returns `401`
- valid bearer token reaches the mocked image provider

Run:

```powershell
cd C:\Users\soulzyn\Desktop\codex\cf-codex-workstation
npm test -- --run
```

Expected: FAIL until endpoint exists.

**Step 2: Write mocked provider success test**

Mock the upstream image provider response and assert:

- response status is `200`
- payload has `status: "ready"`
- payload has `provider: "cloudflare-codex"`
- payload has `imageBase64`
- payload has `mimeType`
- payload has `remoteRequestId`

Do not hit OpenAI or any real image API in tests.

**Step 3: Add Cloudflare env/secrets contract**

Worker secrets/vars:

- `IMAGE_PROXY_TOKEN`
- `OPENAI_IMAGE_API_KEY`
- `OPENAI_IMAGE_BASE_URL`
- `OPENAI_IMAGE_MODEL`
- optional `STATIC_PAGE_VISUAL_MAX_PROMPT_CHARS`

The model value must be read from Cloudflare env. Verify the current official OpenAI image model and endpoint before setting production secrets.

**Step 4: Implement endpoint in workstation**

Endpoint:

```text
POST /api/image/static-page-draft
```

Validation:

- require bearer token
- require non-empty prompt
- enforce max prompt length
- allow only expected `size`, `quality`, and `responseFormat` values
- generate or forward a request id
- return sanitized provider errors

**Step 5: Keep GPT image calls Cloudflare-side**

The workstation endpoint owns:

- image provider auth
- provider request shape
- provider response parsing
- provider latency/error classification

V3 owns:

- visual brief construction
- Cloudflare request metadata
- durable `ReportVisualDraft` state
- local copy persistence
- UI polling and display

**Step 6: Optionally add frontdoor pass-through**

If `codex.souleye.cc` is the stable public endpoint, add a pass-through in:

```text
C:\Users\soulzyn\Desktop\codex\cf-codex-frontdoor\worker.js
```

Keep workstation direct URL usable for diagnostics.

**Step 7: Run tests and deploy separately**

```powershell
cd C:\Users\soulzyn\Desktop\codex\cf-codex-workstation
npm test -- --run
npx wrangler deploy
```

If frontdoor changes:

```powershell
cd C:\Users\soulzyn\Desktop\codex\cf-codex-frontdoor
npx wrangler deploy
```

**Step 8: Commit in Cloudflare repo**

Commit Cloudflare changes in their own repository, not in `ai-data-platform-v3`.

## Task 7: Add Visual Draft Workflow Definition

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/workflow-definitions/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/workflow-definitions/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Assert the workflow catalog contains:

- kind: `StaticPageVisualDraft`
- queue: `static_page_visual`
- task key: `generate_static_page_visual_draft`
- terminal success/failure transitions

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p workflow-definitions static_page_visual -- --nocapture"
```

Expected: FAIL.

**Step 2: Add workflow kind and catalog entry**

Add a new `WorkflowKind` variant with stable wire string `static_page_visual_draft`.

**Step 3: Add runtime inspect hydration placeholder**

`runtime.inspect` should be able to show visual draft execution scope once visual drafts exist.

**Step 4: Run tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p workflow-definitions && cargo test -p platform-api static_page_visual"
```

**Step 5: Commit**

```powershell
git add crates/domain-model/src/lib.rs crates/workflow-definitions/src/lib.rs crates/platform-api/src/lib.rs
git commit -m "Add static page visual draft workflow"
```

## Task 8: Add Visual Draft API Routes

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing route tests**

Tests:

- `POST /v1/report-plans/{plan_id}/visual-drafts` rejects missing plan
- rejects plan with no `current_ast_version_id` unless request allows planning-pending mode
- creates workflow execution and initial `generating` visual draft record
- `GET /v1/report-plans/{plan_id}/visual-drafts` lists newest first
- image endpoint returns 404 until image exists

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api visual_draft_route -- --nocapture"
```

Expected: FAIL.

**Step 2: Implement create route**

Request:

```json
{
  "objective": "Show performance, risks, and recommended next actions.",
  "visual_style": "premium_consulting",
  "aspect_ratio": "desktop_16_9",
  "quality": "medium"
}
```

Response:

```json
{
  "visual_draft": { "status": "generating" },
  "workflow_execution": { "...": "..." }
}
```

**Step 3: Implement list/detail/image routes**

Image endpoint reads the scoped local file and sets `Content-Type` from the draft `mime_type`.

**Step 4: Run tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api visual_draft_route"
```

**Step 5: Commit**

```powershell
git add crates/platform-api/src/lib.rs
git commit -m "Add static page visual draft APIs"
```

## Task 9: Add Static Page Visual Worker

**Files:**

- Create: `crates/static-page-visual-worker/Cargo.toml`
- Create: `crates/static-page-visual-worker/src/main.rs`
- Modify: `Cargo.toml`
- Test: `crates/static-page-visual-worker/src/main.rs`

**Step 1: Write worker helper tests**

Test pure helpers:

- context parsing requires `report_visual_draft_id`
- failed provider result maps to `ReportVisualDraftStatus::Failed`
- successful provider result builds ready manifest with image URL path

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-visual-worker -- --nocapture"
```

Expected: FAIL because crate does not exist.

**Step 2: Implement worker loop**

Follow existing worker style from `report-render-worker`:

- queue: `static_page_visual`
- task key: `generate_static_page_visual_draft`
- load execution
- load draft
- load plan and current AST version
- build visual brief
- call provider
- write image bytes
- mark draft ready or failed
- send workflow signal
- mark task succeeded/failed

**Step 3: Add README worker command later**

Do not update README until the worker builds.

**Step 4: Run tests and workspace check**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-visual-worker && cargo check --workspace"
```

**Step 5: Commit**

```powershell
git add Cargo.toml crates/static-page-visual-worker
git commit -m "Generate static page visual drafts in worker"
```

## Task 10: Add Model-Facing Summary And Runtime Inspect Support

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/validation/runtime-inspect-pretty.md`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Assert:

- ready visual draft has `capability_class=report_planning` or a new explicit visual-draft class only if the protocol needs it
- ready visual draft recommends `report.render`
- failed visual draft has `evidence_state=degraded`
- runtime pretty summary includes `report_visual_draft_id`, status, provider, image path

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api visual_draft_model_facing -- --nocapture"
```

Expected: FAIL.

**Step 2: Implement summary derivation**

Prefer existing protocol vocabulary:

- Ready visual draft: evidence is enough to continue toward `report.render`.
- Failed visual draft: degraded, but plan remains usable.
- Generating visual draft: continuation is waiting.

Only add new enum values if existing `WorkflowModelFacingSummaryView` cannot express the state.

**Step 3: Run tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api visual_draft_model_facing"
```

**Step 4: Commit**

```powershell
git add crates/contracts/src/lib.rs crates/platform-api/src/lib.rs docs/validation/runtime-inspect-pretty.md
git commit -m "Expose visual draft runtime summaries"
```

## Task 11: Build Workbench API Helpers In Web

**Files:**

- Create: `apps/web/app/static-pages/api.js`
- Test manually through web build

**Step 1: Add helpers**

Implement:

- `prepareStaticPageContext(payload)`
- `createReportVisualDraft(planId, payload)`
- `listReportVisualDrafts(planId)`
- `loadReportVisualDraft(draftId)`

Use existing `/api/v3/*` proxy conventions.

**Step 2: Add defensive response handling**

If the API returns a workflow execution but no ready image yet, UI should render generating state and start polling.

**Step 3: Run build**

```powershell
pnpm --filter @ai-data-platform-v3/web build
```

Expected: PASS.

**Step 4: Commit**

```powershell
git add apps/web/app/static-pages/api.js
git commit -m "Add static page workbench web API helpers"
```

## Task 12: Build New Static Page Workbench UI

**Files:**

- Create: `apps/web/app/static-pages/page.js`
- Create: `apps/web/app/components/StaticPageWorkbench.js`
- Create: `apps/web/app/components/StaticPageVisualDraftPanel.js`
- Modify: `apps/web/app/components/Sidebar.js`
- Modify: `apps/web/app/globals.css`

**Step 1: Create the route**

The page should load inside the same V3 visual language, not the old report draft screen.

First viewport:

- dataset/report plan selector
- generated title
- editable objective
- visual style selector
- primary action: generate visual draft
- secondary action: refresh objective

**Step 2: Add visual draft panel**

States:

- empty
- generating
- ready with image
- failed with retry

When ready, show:

- image
- provider/model
- generated time
- primary action: render editable report

**Step 3: Add polling**

Poll every 3 seconds while status is `generating`.

Stop when status is `ready` or `failed`.

**Step 4: Add navigation entry**

Add a stable sidebar item such as `Static page workbench`. Do not remove existing Report Service controls.

**Step 5: Run build**

```powershell
pnpm --filter @ai-data-platform-v3/web build
```

**Step 6: Commit**

```powershell
git add apps/web/app/static-pages apps/web/app/components/StaticPageWorkbench.js apps/web/app/components/StaticPageVisualDraftPanel.js apps/web/app/components/Sidebar.js apps/web/app/globals.css
git commit -m "Add static page visual workbench UI"
```

## Task 13: Connect Visual Draft To Existing Report Render

**Files:**

- Modify: `apps/web/app/static-pages/page.js`
- Modify: `apps/web/app/components/StaticPageWorkbench.js`
- Modify: `crates/platform-api/src/lib.rs` only if route response needs extra fields
- Test: `crates/platform-api/src/lib.rs` if API changes

**Step 1: Reuse existing render endpoint**

After a visual draft is ready, the UI should call:

```text
POST /v1/report-plans/{plan_id}/renders
```

with:

```json
{ "surface": "pc" }
```

Do not create a second render pipeline until the user confirms the image-to-render mismatch is a real problem.

**Step 2: Show render progress separately**

Visual draft status and render output status are separate:

- visual ready
- editable render requested
- editable render output ready
- publish available

**Step 3: Run web build**

```powershell
pnpm --filter @ai-data-platform-v3/web build
```

**Step 4: Commit**

```powershell
git add apps/web/app/static-pages apps/web/app/components/StaticPageWorkbench.js
git commit -m "Connect visual drafts to report rendering"
```

## Task 14: Add Chat Entry Handoff

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/ChatPanel.js`
- Modify: `apps/web/app/static-pages/page.js`
- Optional Modify: `crates/platform-api/src/lib.rs`

**Step 1: Preserve current Report Service gate**

Do not bypass `chat_session.report_entry`. Static page workbench entry should appear after the user confirms Report Service.

**Step 2: Add transition affordance**

When report entry is confirmed and a `confirmed_report_plan_id` exists, expose an action:

```text
Open static page visual workbench
```

Pass `plan_id` and current dataset id through query params:

```text
/static-pages?plan_id=<id>&dataset_id=<id>
```

**Step 3: Keep mobile safe**

On narrow screens, show image preview and status. It is acceptable for editing/publishing affordances to remain desktop-first in MVP.

**Step 4: Run web build**

```powershell
pnpm --filter @ai-data-platform-v3/web build
```

**Step 5: Commit**

```powershell
git add apps/web/app/HomePageClient.js apps/web/app/components/ChatPanel.js apps/web/app/static-pages/page.js
git commit -m "Add chat handoff to static page workbench"
```

## Task 15: Add Cloudflare-Codex Provider Client Behind Flag

**Files:**

- Modify: `crates/static-page-visual-runtime/src/lib.rs`
- Test: `crates/static-page-visual-runtime/src/lib.rs`
- Modify: `README.md`

**Step 1: Verify the Cloudflare endpoint contract**

Before writing V3 provider code, confirm the Cloudflare endpoint is available:

```text
POST https://codex.souleye.cc/api/image/static-page-draft
```

or direct workstation:

```text
POST https://cf-codex-workstation.soulzyn.workers.dev/api/image/static-page-draft
```

V3 should not verify the upstream OpenAI image API directly. Cloudflare owns that provider-specific integration.

**Step 2: Write tests with injected HTTP client**

Do not make tests hit the network.

Test:

- missing remote URL returns unavailable
- missing remote token returns unavailable
- provider sends bearer token to remote URL
- provider forwards configured model/size/quality
- provider parses `imageBase64`, `mimeType`, `remoteRequestId`, and `remoteAssetUrl`
- non-2xx response returns a typed error
- `status: failed` response maps to provider failure

**Step 3: Implement Cloudflare provider**

Support env:

- `STATIC_PAGE_VISUAL_DRAFT_ENABLED`
- `STATIC_PAGE_VISUAL_DRAFT_PROVIDER=cloudflare-codex`
- `STATIC_PAGE_VISUAL_DRAFT_REMOTE_URL`
- `STATIC_PAGE_VISUAL_DRAFT_REMOTE_TOKEN`
- `STATIC_PAGE_VISUAL_IMAGE_MODEL`
- `STATIC_PAGE_VISUAL_IMAGE_SIZE`
- `STATIC_PAGE_VISUAL_IMAGE_QUALITY`

Do not support `openai-direct` in V3 production code. If a local direct provider is ever needed for debugging, keep it behind a separate explicitly named feature flag and do not document it as the normal route.

**Step 4: Persist Cloudflare metadata**

Map response fields into `ReportVisualDraft`:

- `remote_request_id`
- `remote_asset_url`
- `provider`
- `model`
- `revised_prompt`
- `mime_type`

Persist a local image copy from `imageBase64` when present.

**Step 5: Run tests**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-visual-runtime && cargo check --workspace"
```

**Step 6: Commit**

```powershell
git add crates/static-page-visual-runtime/src/lib.rs README.md
git commit -m "Add Cloudflare visual image provider"
```

## Task 16: Documentation And Smoke Test

**Files:**

- Modify: `README.md`
- Modify: `docs/plans/2026-04-23-v3-development-plan.md`
- Create: `docs/validation/static-page-visual-workbench-smoke.md`

**Step 1: Add README worker command**

Add:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo run -p static-page-visual-worker"
```

Document mock provider env:

```powershell
$env:STATIC_PAGE_VISUAL_DRAFT_ENABLED = "true"
$env:STATIC_PAGE_VISUAL_DRAFT_PROVIDER = "mock"
```

Document Cloudflare provider env:

```powershell
$env:STATIC_PAGE_VISUAL_DRAFT_ENABLED = "true"
$env:STATIC_PAGE_VISUAL_DRAFT_PROVIDER = "cloudflare-codex"
$env:STATIC_PAGE_VISUAL_DRAFT_REMOTE_URL = "https://codex.souleye.cc"
$env:STATIC_PAGE_VISUAL_DRAFT_REMOTE_TOKEN = "<token>"
$env:STATIC_PAGE_VISUAL_IMAGE_MODEL = "<configured-on-cloudflare>"
```

Make clear that `OPENAI_IMAGE_API_KEY` belongs in the Cloudflare Worker secret store, not in the V3 platform environment.

**Step 2: Add smoke checklist**

Checklist:

- API starts
- web starts
- report plan exists and is planned
- workbench loads context
- visual draft starts
- mock image appears
- Cloudflare provider can be smoke-tested from V3 with a non-customer prompt after workstation deployment
- report render can be requested after image
- visual draft failure does not block existing Report Service controls

**Step 3: Run final verification**

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check --workspace"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test"
pnpm --filter @ai-data-platform-v3/web build
git diff --check
```

**Step 4: Commit**

```powershell
git add README.md docs/plans/2026-04-23-v3-development-plan.md docs/validation/static-page-visual-workbench-smoke.md
git commit -m "Document static page visual workbench"
```

## Rollout Strategy

1. Ship mock provider UI first.
2. Add and deploy the Cloudflare remote Codex image endpoint in `cf-codex-workstation`.
3. Connect V3 with `STATIC_PAGE_VISUAL_DRAFT_PROVIDER=cloudflare-codex`.
4. Keep OpenAI image keys only in Cloudflare Worker secrets.
5. Review visual workbench UX with deterministic mock images and one controlled Cloudflare image smoke test.
6. Keep existing Report Service control panel as the reliable fallback.
7. Add auto-render-after-image only if manual render feels too slow in customer demos.
8. Add image-to-blueprint parsing only after proving that static report renders diverge too much from visual drafts.

## Final Verification Before Push

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check --workspace"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test"
pnpm --filter @ai-data-platform-v3/web build
git diff --check
```

Expected:

- Rust format passes.
- Workspace check passes.
- Rust tests pass.
- Web build passes.
- `git diff --check` has no whitespace errors. Windows CRLF warnings are acceptable in this repo.
