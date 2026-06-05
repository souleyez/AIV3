# Codex Kernel Separated Memory Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the old OpenClaw-as-main-execution direction with a Codex Mac Host execution-kernel direction, while adding first-class separated memory so browser chats, datasets, projects, Codex task threads, and future external tools cannot leak context into each other accidentally.

**Architecture:** DataMax remains the control plane and source of truth: PostgreSQL owns tenants, local-key visibility, datasets, AssistantRun state, memory boundaries, ReAct validation, runtime audit, and artifacts. Codex Mac Host becomes the preferred execution kernel behind a disabled-by-default external host bridge; OpenClaw stays as an optional provider/legacy sidecar. Memory is split into explicit memory spaces, and every model/tool/Codex task receives only the memory spaces DataMax selected and audited for that run.

**Tech Stack:** Rust crates `domain-model`, `contracts`, `storage`, `platform-api`, `assistant-runtime`, `llm-gateway`, existing ReAct modules, workflow/runtime inspect; Next.js 16 / React 19 in `apps/web`; PostgreSQL 17.9 target; Mac-hosted Codex execution daemon later; optional OpenClaw provider remains fallback-safe.

---

## Why This Plan Exists

The project already completed an OpenClaw first pass:

- `llm-gateway` has an optional `OpenClawLlmProvider`.
- AssistantRun and static-page runtime can use OpenClaw as a provider.
- ReAct has gated `openclaw_memory_recall` and `openclaw_readonly_execution` stubs.

That is enough. OpenClaw should not become the main execution kernel.

The new mainline is:

```text
DataMax Web/API
  -> AssistantRun / Host-Controlled ReAct
  -> DataMax memory-space policy
  -> DataMax workflow/task/audit
  -> Codex Mac Host execution kernel
  -> artifacts and redacted runtime logs back into DataMax
```

Model routing is a separate DataMax-owned concern. Codex is not assumed to provide a production model gateway. Use `docs/plans/2026-05-03-codex-gateway-model-proxy-plan.md` for the detailed boundary between Codex Host, `llm-gateway`, and a future internal `model-proxy`.

The key product feature added by this plan is separated memory:

- Users can run normal chat without selecting a dataset.
- The model knows the system and visible database summary, but not unrelated private/project/thread memory.
- Data-source RAG, conversation memory, project memory, and Codex task working memory are separate scopes.
- The model can ask DataMax to recall or write memory, but DataMax validates the memory space first.
- Codex multi-thread execution uses one isolated task context per task.
- Cross-thread or cross-project memory is never implicit.

## Current Baseline

Existing useful foundations:

- `assistant_runs` already store `local_thread_id`, `selected_scope`, `scope_candidates`, `context_policy`, `evidence_state`, `execution_trail`, `output_artifacts`, and `runtime_manifest`.
- `conversation_memory_items` already store hidden conversation memory by `tenant_id + local_thread_id`.
- Dataset visibility already uses `public/private` semantics and active secret binding ids from `X-AI-Data-Platform-Secret-Binding-Ids`.
- ReAct has typed action parsing, tool routing, protocol repair, and safe UI progress.
- Static-page module edits, previews, and final render worker are already connected to AssistantRun/ReAct.
- Frontend already has a local `local_thread_id`, local chat cache, local activity cache, local secret binding cache, startup briefing, and scope planner.

Important gaps:

- Conversation memory is isolated only by `local_thread_id`; it has no first-class `memory_space_id`.
- Existing memory search is simple recent/`ilike` behavior, not memory-space ranked recall.
- Old `chat_sessions` are still dataset-bound, while new AssistantRun is the real ordinary-chat path.
- Dataset visibility is mostly enforced by API helpers, but still deserves stronger tests and clearer memory-space interaction.
- OpenClaw provider errors may expose raw upstream body and need redaction before any wider external-provider rollout.
- Codex Host execution has not been modeled as a DataMax-owned, allowlisted, audited runtime capability yet.

## Product Semantics

### Memory Space Types

Use these first-version memory spaces:

- `conversation`: one browser conversation/thread. Default for ordinary chat.
- `project`: an explicit shared project/workspace memory that the user or model can select.
- `task`: an ephemeral memory space for one long-running Codex execution task.
- `dataset`: derived memory/catalog for a dataset; this remains backed by existing dataset memory directories and retrieval evidence.
- `system`: product/system memory and safe capability summaries; never stores user private content.

Do not add general team/corporate sharing yet. The local-key model is still intentionally simple.

### Default Rules

- A browser terminal gets a stable `local_thread_id`.
- Each `local_thread_id` gets a default `conversation` memory space.
- A new conversation can get a new `local_thread_id` and memory space without deleting old memory.
- A static-page draft inherits the AssistantRun memory space at creation.
- A Codex host task always gets a separate `task` memory space, linked back to the parent AssistantRun.
- The model can see memory-space summaries, not raw memory, until DataMax decides recall is relevant.
- Generated artifacts are not added to memory by default.
- User utterances may be memory candidates; assistant outputs and generated artifacts are excluded unless explicitly promoted.

### Strict Isolation Rules

```text
No selected memory space -> no hidden user memory supplied.
Conversation memory -> only current local_thread_id unless explicitly linked.
Project memory -> only selected project memory space.
Task memory -> only the Codex task and its parent AssistantRun can read it.
Dataset memory -> only visible selected/inferred datasets can supply it.
OpenClaw memory -> optional evidence only, never a source of truth.
```

## Target Architecture

```mermaid
flowchart LR
  "Browser Terminal" --> "DataMax Web"
  "DataMax Web" --> "AssistantRun API"
  "AssistantRun API" --> "Memory Policy"
  "Memory Policy" --> "Memory Spaces"
  "Memory Policy" --> "Dataset Visibility"
  "AssistantRun API" --> "Host-Controlled ReAct"
  "Host-Controlled ReAct" --> "DataMax Tool Registry"
  "DataMax Tool Registry" --> "Retrieval / Documents / Reports / Static Pages"
  "DataMax Tool Registry" --> "External Host Bridge"
  "External Host Bridge" --> "Codex Mac Host"
  "Codex Mac Host" --> "Task Memory Space"
  "Codex Mac Host" --> "Artifacts / Logs"
  "Artifacts / Logs" --> "Runtime Inspect"
```

## Parallel Development Strategy

Use Codex multi-threading during implementation, but keep write scopes disjoint.

Suggested parallel lanes:

- Lane A: storage/domain/contracts. Owns migrations, structs, repositories, storage tests.
- Lane B: AssistantRun/ReAct/API. Owns memory-space selection, evidence supply, ReAct actions, runtime inspect.
- Lane C: frontend. Owns local memory-space cache, minimal UI, chat payload changes, tests.
- Lane D: Codex Host design/stub. Owns disabled-by-default external host bridge contract and docs.

Do not let multiple workers edit `crates/platform-api/src/lib.rs` at the same time. That file is already large and conflict-prone.

## Task 0: Update The Active Handoff Plan

**Files:**

- Modify: `docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md`
- Reference: `docs/plans/2026-04-29-openclaw-extension-adapter-plan.md`
- Reference: `docs/plans/2026-04-29-DataMax-react-agent-refinement-plan.md`

**Step 1: Mark OpenClaw mainline as completed and downgraded**

Add a short status block:

```markdown
### 2026-05-03 Direction Change: Codex Host Becomes Main Execution Kernel

OpenClaw provider and gated ReAct stubs have completed their first optional-extension pass.
Do not continue OpenClaw as the main execution-kernel route.
OpenClaw remains a provider/legacy sidecar only.
The next execution-kernel work targets Codex Mac Host plus separated memory.
```

**Step 2: Add this plan as the new active architecture plan**

Add `docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md` to the source-plan list.

**Step 3: Verify**

Run:

```powershell
git diff --check
```

Expected: no whitespace errors.

**Step 4: Commit**

```powershell
git add docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md
git commit -m "docs: plan codex kernel separated memory"
```

## Task 1: Add Memory Space Domain Model

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: `crates/domain-model/src/lib.rs`

**Step 1: Add memory-space id and enums**

Add:

```rust
id_type!(MemorySpaceId);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySpaceKind {
    Conversation,
    Project,
    Task,
    Dataset,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySpaceVisibility {
    Private,
    Shared,
    Public,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryItemState {
    Active,
    Archived,
    Expired,
}
```

Add `as_str` and `from_str` implementations following existing enum style.

**Step 2: Add `MemorySpace` struct**

Add:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemorySpace {
    pub id: MemorySpaceId,
    pub tenant_id: TenantId,
    pub key: String,
    pub title: String,
    pub kind: MemorySpaceKind,
    pub visibility: MemorySpaceVisibility,
    pub local_thread_id: Option<String>,
    pub owner_fingerprint: Option<String>,
    pub parent_space_id: Option<MemorySpaceId>,
    pub source_refs: serde_json::Value,
    pub retention_policy: serde_json::Value,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

**Step 3: Extend `ConversationMemoryItem`**

Add fields:

```rust
pub memory_space_id: Option<MemorySpaceId>,
pub source_run_id: Option<AssistantRunId>,
pub source_task_context_id: Option<String>,
pub state: MemoryItemState,
pub importance: f64,
pub expires_at: Option<DateTime<Utc>>,
```

Keep `local_thread_id` for backward compatibility during migration.

**Step 4: Add contract views**

In `crates/contracts/src/lib.rs`, add:

```rust
pub struct MemorySpaceView { ... }
pub struct CreateMemorySpaceRequest { ... }
pub struct UpdateMemorySpaceRequest { ... }
pub struct MemoryRecallRequest { ... }
pub struct MemoryRecallResponse { ... }
```

Use JSON-friendly fields:

```rust
pub kind: String,
pub visibility: String,
pub local_thread_id: Option<String>,
pub owner_fingerprint: Option<String>,
pub metadata: Value,
```

**Step 5: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p domain-model memory_space"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p contracts memory"
```

Expected: compile and focused tests pass.

## Task 2: Add Storage Schema And Repository

**Files:**

- Modify: `crates/storage/src/lib.rs`
- Create: `crates/storage/migrations/0003_memory_spaces.sql`
- Modify if needed: `crates/storage/migrations/0001_initial_schema.sql`
- Test: `crates/storage/src/lib.rs`

**Important migration note:** current `PgStorage::migrate()` only executes `INITIAL_SCHEMA.sql`, while `0002_workflow_execution_runtime_records.sql` exists but is not part of a migration array. Fix this before adding more migration files, or the new migration will be dead code.

**Step 1: Add migration list**

Replace single-schema migrate with an ordered array:

```rust
pub const MIGRATIONS: &[Migration] = &[
    INITIAL_SCHEMA,
    Migration {
        version: "0002",
        description: "workflow execution runtime records",
        sql: include_str!("../migrations/0002_workflow_execution_runtime_records.sql"),
    },
    Migration {
        version: "0003",
        description: "memory spaces and separated conversation memory",
        sql: include_str!("../migrations/0003_memory_spaces.sql"),
    },
];
```

Then:

```rust
pub async fn migrate(&self) -> Result<()> {
    for migration in MIGRATIONS {
        sqlx::raw_sql(migration.sql).execute(&self.pool).await?;
    }
    Ok(())
}
```

**Step 2: Create `memory_spaces` table**

`0003_memory_spaces.sql`:

```sql
create table if not exists memory_spaces (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    key text not null,
    title text not null,
    kind text not null,
    visibility text not null default 'private',
    local_thread_id text,
    owner_fingerprint text,
    parent_space_id uuid references memory_spaces (id) on delete set null,
    source_refs jsonb not null default '{}'::jsonb,
    retention_policy jsonb not null default '{}'::jsonb,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    archived_at timestamptz,
    unique (tenant_id, key)
);

create index if not exists memory_spaces_tenant_kind_idx
    on memory_spaces (tenant_id, kind, updated_at desc);

create index if not exists memory_spaces_thread_idx
    on memory_spaces (tenant_id, local_thread_id)
    where local_thread_id is not null;
```

**Step 3: Extend conversation memory table**

In `0003_memory_spaces.sql`:

```sql
alter table conversation_memory_items
    add column if not exists memory_space_id uuid references memory_spaces (id) on delete set null,
    add column if not exists source_run_id uuid references assistant_runs (id) on delete set null,
    add column if not exists source_task_context_id text,
    add column if not exists state text not null default 'active',
    add column if not exists importance double precision not null default 0.5,
    add column if not exists expires_at timestamptz;

create index if not exists conversation_memory_items_space_idx
    on conversation_memory_items (tenant_id, memory_space_id, updated_at desc)
    where memory_space_id is not null;

create index if not exists conversation_memory_items_space_summary_idx
    on conversation_memory_items using gin (to_tsvector('simple', summary));
```

If PostgreSQL rejects the expression index because of immutable requirements in the target setup, drop the full-text index from first pass and keep lexical search.

**Step 4: Backfill default conversation memory spaces**

Add idempotent backfill:

```sql
insert into memory_spaces (
    tenant_id,
    key,
    title,
    kind,
    visibility,
    local_thread_id,
    metadata
)
select distinct
    tenant_id,
    'conversation:' || local_thread_id,
    'Conversation ' || local_thread_id,
    'conversation',
    'private',
    local_thread_id,
    jsonb_build_object('backfilled', true)
from conversation_memory_items
where local_thread_id is not null
on conflict (tenant_id, key) do nothing;

update conversation_memory_items item
set memory_space_id = space.id
from memory_spaces space
where item.tenant_id = space.tenant_id
  and item.local_thread_id = space.local_thread_id
  and space.kind = 'conversation'
  and item.memory_space_id is null;
```

**Step 5: Add repositories**

Add:

```rust
pub struct PgMemorySpaceRepository { pool: PgPool }
```

Methods:

```rust
create(tenant_id, &NewMemorySpace) -> Result<MemorySpace>
ensure_conversation_space(tenant_id, local_thread_id, owner_fingerprint) -> Result<MemorySpace>
get_by_id(tenant_id, memory_space_id) -> Result<Option<MemorySpace>>
list_visible(tenant_id, filter) -> Result<Vec<MemorySpace>>
archive(tenant_id, memory_space_id) -> Result<Option<MemorySpace>>
```

Extend `PgConversationMemoryItemRepository`:

```rust
list_by_memory_space(tenant_id, memory_space_id, query, limit)
create(...) // bind memory_space_id and new fields
```

**Step 6: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage memory_space"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage conversation_memory"
```

Expected:

- Existing conversation memory tests still pass.
- Backfilled items resolve a memory space.
- New memory-space repository tests pass.

## Task 3: Add Memory Space API

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Add routes**

Add routes:

```rust
GET    /v1/memory-spaces
POST   /v1/memory-spaces
GET    /v1/memory-spaces/{memory_space_id}
POST   /v1/memory-spaces/{memory_space_id}/archive
GET    /v1/memory-spaces/{memory_space_id}/items
POST   /v1/memory-spaces/{memory_space_id}/items
POST   /v1/memory-spaces/ensure-conversation
```

**Step 2: Implement local-thread defaulting**

`ensure-conversation` accepts:

```json
{
  "local_thread_id": "browser-thread-...",
  "owner_fingerprint": "optional-local-secret-or-device-fingerprint"
}
```

It returns the memory space and should be idempotent.

**Step 3: Protect memory-space visibility**

First version:

- `conversation` spaces require matching `local_thread_id`.
- `task` spaces are not listable from the generic UI unless linked to current AssistantRun.
- `project` spaces are listable only if private to current owner fingerprint or marked shared/public.
- `system` spaces are read-only.
- `dataset` spaces are derived and read-only from this API.

Do not rely on the model to respect this. Enforce it in handlers.

**Step 4: Update conversation memory endpoints**

Existing endpoints must keep working:

```text
GET/POST /v1/conversation-memory-items
```

But internally:

- If `memory_space_id` is supplied, use it after validation.
- If only `local_thread_id` is supplied, call `ensure_conversation_space`.
- Store both `local_thread_id` and `memory_space_id`.

**Step 5: Tests**

Add tests:

```rust
#[tokio::test]
async fn ensure_conversation_memory_space_is_idempotent() { ... }

#[tokio::test]
async fn conversation_memory_items_are_separated_by_memory_space() { ... }

#[tokio::test]
async fn memory_space_archive_hides_future_recall() { ... }

#[tokio::test]
async fn task_memory_space_is_not_listed_as_normal_user_memory() { ... }
```

**Step 6: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api memory_space"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api conversation_memory"
```

Expected: all focused tests pass.

## Task 4: Attach Memory Spaces To AssistantRun

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Extend request/response contracts**

Extend `CreateAssistantRunRequest` and `ContinueAssistantRunRequest`:

```rust
pub memory_space_id: Option<MemorySpaceId>,
pub memory_policy_hint: Option<Value>,
```

Extend response:

```rust
pub memory_space: Option<MemorySpaceView>,
pub memory_candidates: Vec<MemorySpaceView>,
```

**Step 2: Extend `assistant_runs`**

Add nullable columns by migration:

```sql
alter table assistant_runs
    add column if not exists memory_space_id uuid references memory_spaces (id) on delete set null,
    add column if not exists task_context_id text;

create index if not exists assistant_runs_memory_space_idx
    on assistant_runs (tenant_id, memory_space_id, updated_at desc)
    where memory_space_id is not null;
```

Also update `AssistantRun` struct and mapping.

**Step 3: Select memory space during run creation**

Algorithm:

```text
if request.memory_space_id is present:
  validate and use it
else if local_thread_id is present:
  ensure default conversation memory space
else:
  no hidden memory space
```

Do not auto-select project memory based only on weak string matching in the first version. Return project candidates but require user/model action to select.

**Step 4: Update evidence build**

Change `build_assistant_run_evidence_state(...)` to accept:

```rust
memory_space_id: Option<MemorySpaceId>
```

Then recall conversation memory from `memory_space_id` instead of only `local_thread_id`.

Keep old fallback:

```text
if memory_space_id is none and local_thread_id requests memory:
  use old local_thread_id path
```

**Step 5: Update startup/provider input**

Model should receive a concise summary:

```text
当前记忆空间: conversation / project / none
是否已召回记忆: yes/no
可选项目记忆: titles only, no raw content
规则: 不要假设未召回记忆；需要时请求 recall_conversation_memory
```

**Step 6: Tests**

Add tests:

```rust
#[tokio::test]
async fn assistant_run_uses_explicit_memory_space() { ... }

#[tokio::test]
async fn assistant_run_defaults_to_thread_memory_space() { ... }

#[tokio::test]
async fn assistant_run_does_not_recall_other_thread_memory() { ... }

#[tokio::test]
async fn assistant_run_returns_memory_candidates_without_raw_content() { ... }
```

**Step 7: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_memory"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run"
```

Expected: focused AssistantRun tests pass and existing behavior remains compatible.

## Task 5: Add ReAct Memory Actions

**Files:**

- Modify: `crates/platform-api/src/react_agent_contract.rs`
- Modify: `crates/platform-api/src/react_agent_catalog.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_tools.rs`

**Step 1: Add action types**

Add:

```rust
CreateMemorySpace,
SelectMemorySpace,
RecallMemorySpace,
WriteMemoryNote,
ArchiveMemorySpace,
```

JSON action names:

```text
create_memory_space
select_memory_space
recall_memory_space
write_memory_note
archive_memory_space
```

Keep `recall_conversation_memory` as an alias for `recall_memory_space` against the active memory space.

**Step 2: Update weak planning catalog**

Advertise:

```json
{
  "memory_spaces": {
    "mode": "host_controlled",
    "actions": [
      "select_memory_space",
      "recall_memory_space",
      "write_memory_note"
    ],
    "policy": "DataMax validates memory-space visibility; model cannot read cross-thread memory implicitly."
  }
}
```

**Step 3: Implement action validation**

Rules:

- `create_memory_space`: allow `conversation` and `project`; deny `system`, deny arbitrary `dataset`, deny `task` unless a host task is being created.
- `select_memory_space`: only visible memory spaces.
- `recall_memory_space`: only active selected memory spaces.
- `write_memory_note`: only user-approved or clearly user-provided durable facts; first pass should default to writing user utterance summaries, not assistant conclusions.
- `archive_memory_space`: require explicit user wording or UI action.

**Step 4: Redacted observations**

Return observations like:

```json
{
  "status": "completed",
  "action_type": "recall_memory_space",
  "memory_space_id": "...",
  "returned_count": 3,
  "items": [
    {
      "type": "memory_item",
      "summary": "用户之前要求静态页模块可调。",
      "source": "conversation",
      "created_at": "..."
    }
  ]
}
```

Do not return hidden memory from denied spaces.

**Step 5: Tests**

Add tests:

```rust
fn react_recall_memory_space_rejects_unselected_space() { ... }
fn react_recall_memory_space_returns_only_active_items() { ... }
fn react_write_memory_note_uses_current_space() { ... }
fn react_archive_memory_space_requires_explicit_permission() { ... }
```

**Step 6: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api react_agent_contract"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api react_agent_tools"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react"
```

Expected: ReAct tests pass; existing OpenClaw gated tests still pass.

## Task 6: Frontend Memory-Space UX

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/lib/platform-api.js`
- Modify: `apps/web/app/components/ChatPanel.js`
- Modify: `apps/web/app/components/Sidebar.js`
- Create: `apps/web/app/lib/memory-space.js`
- Create: `apps/web/app/lib/memory-space.test.mjs`

**Step 1: Add local storage keys**

Add:

```javascript
const LOCAL_MEMORY_SPACE_ID_STORAGE_KEY = 'aidp-DataMax-memory-space-id';
const LOCAL_MEMORY_SPACE_CACHE_STORAGE_KEY = 'aidp-DataMax-memory-space-cache';
```

**Step 2: Add helper module**

`apps/web/app/lib/memory-space.js`:

```javascript
export function selectDefaultMemorySpace({ localThreadId, spaces = [] }) {
  const conversation = spaces.find((space) =>
    space?.kind === 'conversation' && space?.local_thread_id === localThreadId
  );
  return conversation || null;
}

export function summarizeMemorySpaceForUi(space) {
  if (!space) {
    return '未启用记忆';
  }
  const kindLabel = {
    conversation: '当前会话记忆',
    project: '项目记忆',
    task: '任务记忆',
    dataset: '数据集记忆',
    system: '系统记忆',
  }[space.kind] || '记忆';
  return `${kindLabel} · ${space.title || space.key || '未命名'}`;
}
```

**Step 3: Ensure conversation memory space on startup**

After `localThreadId` is known, call:

```text
POST /api/v3/memory-spaces/ensure-conversation
```

Store the returned `memory_space_id` locally.

If this fails, chat should still work with no hidden memory.

**Step 4: Send memory space with AssistantRun**

When creating or continuing AssistantRun, include:

```javascript
memory_space_id: activeMemorySpace?.id || null,
memory_policy_hint: {
  mode: 'intent_gated',
  currentMemorySpaceKind: activeMemorySpace?.kind || 'none',
}
```

**Step 5: Keep UI simple**

Do not add a big memory management panel.

Add a small status line near the composer:

```text
记忆：当前会话记忆 · 浏览器线程
```

Add two lightweight actions:

- `新会话`: creates a new `local_thread_id` and new conversation memory space.
- `项目记忆`: small menu to select/create a project memory space later; first pass can show read-only current status if creation API exists but UI is not polished.

Natural-language control remains primary:

- “这件事单独记”
- “不要用上个项目的记忆”
- “切到客户 A 的项目记忆”

These should go through ReAct memory actions, not a large form.

**Step 6: Tests**

Add tests:

```javascript
test('selectDefaultMemorySpace chooses matching local thread conversation space', () => { ... });
test('summarizeMemorySpaceForUi labels current conversation memory', () => { ... });
test('summarizeMemorySpaceForUi handles missing space', () => { ... });
```

**Step 7: Verify**

Run:

```powershell
node --test apps/web/app/lib/memory-space.test.mjs apps/web/app/lib/scope-planner.test.mjs apps/web/app/lib/assistant-startup-briefing.test.mjs
cd apps/web
npm run build
```

Expected: tests and build pass.

## Task 7: Add Codex Host Execution Contract

**Status 2026-05-07:** first queue bridge implemented using `docs/architecture/codex-host-bridge-contract.md`. ReAct recognizes `codex_host_task`, rejects it by default, requires `CODEX_HOST_TASK_ENABLED=true`, requires `CODEX_HOST_TASK_ALLOWLIST`, creates `codex_host_task_workflow`, and enqueues `codex_host/run_codex_host_task` for continued AssistantRuns. `crates/codex-host-agent` can claim that task, complete it in `dry_run`, or build a redacted `plan_only` Codex command summary. No local Codex process is launched.

**Files:**

- Create: `docs/architecture/codex-host-execution-kernel.md`
- Modify: `docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md`
- Modify: `crates/platform-api/src/react_agent_contract.rs`
- Modify: `crates/platform-api/src/react_agent_catalog.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Test: `crates/platform-api/src/react_agent_tools.rs`

**Step 1: Document the boundary**

Create architecture doc with:

```text
Codex Host is an execution worker, not a data authority.
DataMax signs and scopes every task.
Codex Host receives a task memory space and explicit artifact workspace.
Codex Host cannot see browser local secret values.
Codex Host cannot read private datasets except through DataMax-supplied evidence/artifacts.
Codex Host task logs are redacted before UI display.
```

**Step 2: Add action type**

Add ReAct action:

```text
codex_host_task
```

First-version arguments:

```json
{
  "capability": "inspect_runtime|run_readonly_check|produce_artifact",
  "task_summary": "short user-visible task",
  "workspace_scope": {
    "kind": "none|project|artifact",
    "allowed_paths": []
  },
  "requires_confirmation": true
}
```

**Step 3: Default disabled behavior**

Implement a gated stub:

- Env flag: `CODEX_HOST_TASK_ENABLED=false` by default.
- Allowlist: `CODEX_HOST_TASK_ALLOWLIST`.
- If disabled, return rejected observation: `codex_host_execution_disabled`.
- If enabled in first pass, enqueue the dedicated workflow task and let `crates/codex-host-agent` complete dry-run until actual host execution is implemented.

**Step 4: Allowlist**

Only allow:

```text
inspect_project
summarize_runtime
run_readonly_check
produce_artifact
```

Deny:

```text
shell_write
git_push
deploy
database_write
secret_read
arbitrary_browser_control
```

**Step 5: Tests**

Add tests:

```rust
fn codex_host_task_is_disabled_by_default() { ... }
fn codex_host_task_rejects_non_allowlisted_capability() { ... }
fn codex_host_task_preflight_allows_enabled_allowlisted_capability() { ... }
```

**Step 6: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api codex_host"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api react_agent_tools"
```

Expected: disabled/default safety behavior passes.

## Task 8: Add Codex Task Memory Space Semantics

**Status 2026-05-07:** partially covered at the workflow-context level only. The queued Codex Host task carries an isolated `task_memory_policy`, parent AssistantRun id, local thread id, capability, and bounded task text. First-class `task_memory_space_id` creation is still pending the memory-space storage foundation.

**Files:**

- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Create task memory space when Codex task is accepted**

When `codex_host_task` is accepted, create:

```text
kind = task
visibility = private
parent_space_id = current AssistantRun memory_space_id
source_refs = { assistant_run_id, react_step, capability }
```

**Step 2: Store task context id**

Generate:

```text
task_context_id = codex-task-{uuid}
```

Return it in observation:

```json
{
  "status": "queued",
  "action_type": "codex_host_task",
  "task_context_id": "codex-task-...",
  "task_memory_space_id": "...",
  "capability": "run_readonly_check"
}
```

**Step 3: Prevent implicit task memory recall**

Task memory should not appear in ordinary conversation recall unless:

- It is the active task context, or
- The task explicitly wrote a promoted summary to the parent conversation/project memory space.

**Step 4: Tests**

Add tests:

```rust
#[tokio::test]
async fn codex_task_gets_separate_task_memory_space() { ... }

#[tokio::test]
async fn task_memory_is_not_recalled_by_parent_conversation_by_default() { ... }

#[tokio::test]
async fn task_summary_can_be_promoted_to_parent_memory() { ... }
```

**Step 5: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api task_memory"
```

Expected: task memory isolation tests pass.

## Task 9: Runtime Inspect And Audit For Separated Memory

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/bin/runtime-inspect-cli.rs`
- Modify: `docs/validation/runtime-inspect-pretty.md`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Extend AssistantRun runtime manifest**

Add:

```json
{
  "memory": {
    "active_memory_space_id": "...",
    "active_memory_space_kind": "conversation",
    "candidate_count": 2,
    "recalled_count": 3,
    "denied_count": 0,
    "task_context_id": null
  }
}
```

**Step 2: Add execution-trail steps**

Examples:

```json
{
  "status": "completed",
  "label": "选择记忆空间",
  "memory_space_kind": "conversation"
}
```

```json
{
  "status": "completed",
  "label": "召回分隔记忆",
  "returned_count": 3
}
```

```json
{
  "status": "rejected",
  "label": "拒绝跨记忆空间访问",
  "reason": "memory_space_not_visible"
}
```

**Step 3: Pretty CLI output**

Add section:

```text
Memory Runtime
- active_space: conversation / title
- recalled: 3
- candidates: 2
- denied: 0
- task_context: none
```

**Step 4: Tests**

Add:

```rust
#[tokio::test]
async fn runtime_inspect_includes_memory_runtime_summary() { ... }
```

**Step 5: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api runtime_inspect_memory"
```

Expected: runtime inspect includes memory summaries without raw denied memory.

## Task 10: Provider And Host Error Redaction

**Files:**

- Modify: `crates/llm-gateway/src/lib.rs`
- Modify: `crates/platform-api/src/react_agent_tools.rs`
- Test: `crates/llm-gateway/src/lib.rs`
- Test: `crates/platform-api/src/react_agent_tools.rs`

**Step 1: Add redaction helper**

Add helper:

```rust
fn redact_provider_error_body(body: &str) -> String {
    let without_tokens = body
        .replace("Authorization", "[redacted-header]")
        .replace("api_key", "[redacted-key]");
    truncate_for_error(&without_tokens, 500)
}
```

Do not use literal-only replacement for final implementation; add case-insensitive redaction for:

- bearer tokens
- access tokens
- api keys
- cookies
- local paths containing secret filenames

**Step 2: Apply to OpenClaw provider errors**

At OpenClaw non-success HTTP paths, use redacted body only.

**Step 3: Apply to future Codex host stub**

Any host task observation should use:

```text
safe_message
redacted_log_excerpt
artifact_refs
```

Never return raw stdout/stderr in UI-facing trail.

**Step 4: Tests**

Add tests:

```rust
fn provider_error_redacts_bearer_token() { ... }
fn provider_error_truncates_large_body() { ... }
fn codex_host_observation_does_not_expose_raw_log() { ... }
```

**Step 5: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p llm-gateway redacts"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api codex_host"
```

Expected: no raw token-like string is emitted.

## Task 11: Dataset Visibility And Memory Interaction Hardening

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Add tests for private dataset memory separation**

Cases:

```rust
#[tokio::test]
async fn private_dataset_memory_not_visible_without_secret_binding() { ... }

#[tokio::test]
async fn project_memory_cannot_reveal_hidden_dataset_title() { ... }

#[tokio::test]
async fn selected_scope_cannot_reference_hidden_dataset_through_memory_action() { ... }
```

**Step 2: Make memory items carry source visibility**

When memory is created from dataset-derived material, metadata must include:

```json
{
  "source_visibility": "public|private",
  "source_dataset_ids": ["..."],
  "source_secret_binding_ids": ["..."]
}
```

Before supplying a memory item, DataMax must verify current active secret binding ids still satisfy the source visibility.

**Step 3: Do not over-implement sharing**

First pass may reject project memory items with private dataset refs unless the active secret header matches. That is safer than inventing team sharing too early.

**Step 4: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api dataset_visibility"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api memory_space"
```

Expected: hidden private data stays hidden through memory routes.

## Task 12: Assistant Startup Briefing Update

**Files:**

- Modify: `apps/web/app/lib/assistant-startup-briefing.js`
- Modify: `apps/web/app/lib/assistant-startup-briefing.test.mjs`
- Modify: `crates/platform-api/src/lib.rs`

**Step 1: Include memory capability without raw memory**

Frontend briefing should include:

```javascript
{
  memoryPolicy: 'intent_gated_separated_memory',
  currentMemorySpaceKind: 'conversation',
  currentMemorySpaceTitle: '当前会话',
  memoryRules: [
    '未召回的记忆不可假设',
    '跨项目/跨线程记忆需要明确选择',
    '生成物默认不是记忆'
  ]
}
```

**Step 2: Backend provider input should mirror this**

Provider input must state:

```text
DataMax uses separated memory spaces. You may request memory recall through host actions, but you must not assume unrelated thread/project memory.
```

**Step 3: Tests**

Add:

```javascript
test('startup briefing describes separated memory without raw memory items', () => { ... });
```

**Step 4: Verify**

Run:

```powershell
node --test apps/web/app/lib/assistant-startup-briefing.test.mjs
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_provider_input"
```

Expected: model-facing briefing is concise and does not leak raw memory content.

## Task 13: Codex Host Daemon Plan Stub

**Files:**

- Create: `docs/architecture/codex-host-daemon-contract.md`
- Create: `docs/operations/codex-host-mac-setup.md`
- Modify: `docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md`

**Step 1: Define daemon contract**

Document a future daemon:

```text
codex-runtime-agent
  - polls DataMax/Cloudflare queue
  - leases one task
  - starts one Codex execution thread
  - receives scoped prompt/materials only
  - writes logs/artifacts into a task workspace
  - redacts and reports summary back to DataMax
```

**Step 2: Define task payload**

Example:

```json
{
  "task_context_id": "codex-task-...",
  "assistant_run_id": "...",
  "memory_space_id": "...",
  "capability": "run_readonly_check",
  "materials": [
    {
      "type": "artifact",
      "id": "...",
      "safe_url": "..."
    }
  ],
  "instructions": "Run readonly verification and summarize failures.",
  "denied_capabilities": ["secret_read", "database_write", "deploy"]
}
```

**Step 3: Define Mac host safety**

Rules:

- No public unauthenticated port.
- No browser local secret extraction.
- No direct PostgreSQL credentials unless running a DataMax-owned worker role.
- No arbitrary command execution from user text.
- Workspace cleanup must follow local backup-first deletion policy.

**Step 4: Verify**

Run:

```powershell
git diff --check
```

Expected: docs clean.

## Task 14: Backward Compatibility And Migration Smoke

**Files:**

- Modify tests only unless bugs are found:
  - `crates/platform-api/src/lib.rs`
  - `crates/storage/src/lib.rs`
  - `apps/web/app/lib/memory-space.test.mjs`

**Step 1: Existing ordinary chat**

Verify:

```text
No selected dataset + current conversation memory space -> normal model chat, no forced RAG.
```

**Step 2: Existing selected dataset**

Verify:

```text
Selected dataset -> retrieval evidence still supplied.
Memory space does not replace selected-scope semantics.
```

**Step 3: Static page flow**

Verify:

```text
Create static-page draft -> inherits active memory space and selected scope.
Module edits -> do not mutate memory unless explicitly written.
Image/final render -> artifacts do not become memory by default.
```

**Step 4: Existing OpenClaw provider**

Verify:

```text
ASSISTANT_RUN_RUNTIME_PROVIDER=openclaw still works when extension is enabled.
OpenClaw bridge stubs remain disabled by default.
```

**Step 5: Commands**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all --check"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage memory"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_react"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api memory_space"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api openclaw_react"
node --test apps/web/app/lib/memory-space.test.mjs apps/web/app/lib/scope-planner.test.mjs apps/web/app/lib/assistant-startup-briefing.test.mjs
cd apps/web
npm run build
```

Expected: all focused checks pass.

## Task 15: Commit And Handoff

**Files:**

- Modify: `docs/plans/2026-04-29-DataMax-consolidated-development-handoff-plan.md`
- Add/modify implementation files from previous tasks
- Add: this plan file

**Step 1: Review diff**

Run:

```powershell
git status --short
git diff --check
git diff --stat
```

Expected:

- No secret files.
- No `.storage`.
- No generated access keys.
- No unrelated cleanup.

**Step 2: Commit in slices**

Recommended commits:

```powershell
git commit -m "docs: plan codex kernel separated memory"
git commit -m "feat(storage): add separated memory spaces"
git commit -m "feat(platform-api): scope assistant runs to memory spaces"
git commit -m "feat(platform-api): add react memory-space actions"
git commit -m "feat(web): show current separated memory scope"
git commit -m "feat(platform-api): add gated codex host task action"
git commit -m "docs: define codex mac host daemon contract"
```

**Step 3: Update handoff**

At the end of implementation, add a status block to this plan and to the consolidated handoff:

```markdown
**Status 2026-05-xx:** separated memory first pass completed.
Conversation memory now has first-class memory spaces, AssistantRun stores active memory space,
ReAct can recall/write/select memory through DataMax validation, and Codex host task stubs allocate isolated task memory.
```

## Acceptance Criteria

- Existing ordinary chat works with no selected dataset and no explicit memory.
- Existing selected dataset/RAG semantics are unchanged.
- A browser thread has a default conversation memory space.
- Two browser threads cannot see each other's conversation memory unless explicitly linked through a project memory space.
- ReAct can request memory recall, but DataMax validates the memory space before returning items.
- Codex host task stubs create isolated task memory spaces and do not leak into conversation memory.
- Runtime inspect shows memory-space decisions and denied cross-space requests.
- OpenClaw remains optional and disabled/fallback-safe.
- Provider and host errors are redacted before entering UI/runtime summaries.
- Frontend exposes memory status simply without turning the product into a memory-management form.

## Non-Goals

- No full team permission system in this slice.
- No arbitrary shared memory graph in this slice.
- No direct Codex daemon implementation before the disabled DataMax host-action contract is safe.
- No automatic promotion of generated artifacts into memory.
- No direct model access to PostgreSQL, browser local secret values, or raw host logs.

## Recommended Next Thread Prompt

```text
Continue DataMax from docs/plans/2026-05-03-codex-kernel-separated-memory-plan.md.
OpenClaw provider/stubs already completed their optional first pass; do not continue OpenClaw as the main execution kernel.
Start with Task 0 and Task 1: update the consolidated handoff, then add memory-space domain/storage foundation.
Keep DataMax as the control plane. Codex Host is the future execution kernel, but first implementation must be disabled-by-default and audited.
Use separated memory rules: conversation/project/task/dataset/system memory must not leak across scopes.
```
