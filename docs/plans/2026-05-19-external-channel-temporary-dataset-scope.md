# External Channel Temporary Dataset Scope Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let each third-party conversation event provide a document range that V3 treats as a temporary dataset-like scope for that conversation, without moving or duplicating canonical documents.

**Architecture:** Keep `documents.dataset_id` as the canonical ingest/source dataset. P0 adds a virtual temporary dataset scope inside `selected_scope` and constrains retrieval to the resolved document IDs already supplied by the third party. P1 adds durable dataset-document membership so one physical document can appear in multiple formal or temporary datasets with TTL/cleanup semantics.

**Tech Stack:** Rust workspace, `platform-api`, `contracts`, `storage`, PostgreSQL, existing AssistantRun selected-scope and retrieval-evidence pipeline.

---

## Design Rules

- Do not update `documents.dataset_id` to build a temporary scope.
- Do not copy document blobs, chunks, or retrieval evidence for P0.
- Treat third-party `available_document_external_ids` as a conversation-scoped allowlist.
- If a document is still parsing, failed, or re-parsing, surface that state to the model inside the same temporary scope.
- P0 must be schema-free and deployment-light.
- P1 may introduce a membership table, but only after P0 behavior is stable.

## Target Scope Shape

P0 should enrich external-channel selected scope like this:

```json
{
  "type": "external_channel",
  "mode": "external_document_scope",
  "temporary_dataset": {
    "key": "external-session-generic-chat-main-conv-20260518-0001",
    "source": "available_document_external_ids",
    "document_count": 2,
    "restores_on": "conversation_turn_end"
  },
  "available_document_source_id": "src-docs",
  "available_document_external_ids": ["doc-a", "doc-b"],
  "documents": [
    {"type": "document", "id": "<v3-document-id>", "document_external_id": "doc-a"}
  ],
  "datasets": [
    {"type": "dataset", "id": "<canonical-dataset-id>"}
  ],
  "external_document_scope_status": "resolved"
}
```

The `datasets` entry remains the canonical dataset only to let existing retrieval code find candidate chunks. The `documents` list is the actual allowlist.

## Task 1: Lock Current Contract With Tests

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: existing module tests near `external_channel_document_scope_infers_source_from_document_external_id`

**Step 1: Write the failing test**

Add a test named:

```rust
#[tokio::test]
async fn external_channel_document_scope_builds_temporary_dataset_scope_from_available_documents()
```

Test setup:
- Create one canonical dataset.
- Create two documents in that dataset with `external_source` metadata.
- Build an `ExternalBotMessageView` with `conversation_external_id`, `available_document_external_ids`, and `available_document_source_id`.
- Call `enrich_external_channel_document_scope`.

Expected:
- `selected_scope["mode"] == "external_document_scope"`.
- `selected_scope["temporary_dataset"]["document_count"] == 2`.
- `selected_scope["temporary_dataset"]["key"]` is non-empty and stable from channel/conversation/source.
- `selected_scope["documents"]` contains only resolved documents.
- `selected_scope["datasets"]` contains the canonical dataset ID.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p platform-api external_channel_document_scope_builds_temporary_dataset_scope_from_available_documents --lib
```

Expected: FAIL because temporary scope fields do not exist yet.

**Step 3: Implement minimal enrichment**

Modify `enrich_external_channel_document_scope` in `crates/platform-api/src/lib.rs`.

Add helper functions:

```rust
fn external_channel_temporary_dataset_key(
    connection_id: &str,
    message: &ExternalBotMessageView,
    source_id: Option<&str>,
) -> String
```

```rust
fn set_external_channel_temporary_dataset_scope(
    selected_scope: &mut Value,
    connection_id: &str,
    message: &ExternalBotMessageView,
    source_id: Option<&str>,
    document_count: usize,
)
```

Use existing JSON helpers such as `set_payload_value` and existing scope mutation style.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p platform-api external_channel_document_scope_builds_temporary_dataset_scope_from_available_documents --lib
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/platform-api/src/lib.rs
git commit -m "Add temporary dataset scope for external document ranges"
```

## Task 2: Prove Canonical Document Ownership Is Not Mutated

**Files:**
- Modify: `crates/platform-api/src/lib.rs`

**Step 1: Write the failing test**

Add:

```rust
#[tokio::test]
async fn external_channel_temporary_scope_does_not_mutate_document_dataset()
```

Setup:
- Create a canonical dataset and document.
- Enrich external scope with that document.
- Reload document from storage.

Expected:
- `document.dataset_id` is still the original canonical dataset.
- No new document is created.
- No document chunk or retrieval evidence needs to be copied.

**Step 2: Run test**

```bash
cargo test -p platform-api external_channel_temporary_scope_does_not_mutate_document_dataset --lib
```

Expected: PASS after Task 1 if implementation did not mutate storage.

**Step 3: Commit if code changed**

```bash
git add crates/platform-api/src/lib.rs
git commit -m "Test external temporary scope preserves canonical documents"
```

## Task 3: Ensure Retrieval Is Limited To The Supplied Document Range

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Relevant functions:
  - `build_assistant_run_evidence_state`
  - `selected_document_ids_from_scope`
  - `search_dataset_retrieval_with_state`

**Step 1: Write the failing test**

Add:

```rust
#[tokio::test]
async fn assistant_run_external_temporary_scope_retrieval_limits_to_selected_documents()
```

Setup:
- Create one canonical dataset.
- Create document A with chunk text `"Alpha approval policy"`.
- Create document B with chunk text `"Beta payroll secret"`.
- Add retrieval evidence for both.
- Build selected scope with:
  - canonical dataset in `datasets`
  - only document A in `documents`
  - `temporary_dataset` present
- Ask query containing words from document B.

Expected:
- Evidence state does not include document B.
- Evidence state records selected document count.
- Model supply still says retrieval was scoped to the temporary document range.

**Step 2: Run test to verify failure or current behavior**

```bash
cargo test -p platform-api assistant_run_external_temporary_scope_retrieval_limits_to_selected_documents --lib
```

Expected: If current selected-document filtering already works, PASS; otherwise FAIL with document B evidence leaking.

**Step 3: Implement minimal fix if needed**

If evidence leaks, update `build_assistant_run_evidence_state` so after retrieving by canonical dataset it intersects candidates with `selected_document_ids_from_scope(selected_scope)` whenever that list is non-empty.

**Step 4: Run focused tests**

```bash
cargo test -p platform-api assistant_run_external_temporary_scope_retrieval_limits_to_selected_documents --lib
cargo test -p platform-api assistant_run_supplies_ranked_retrieval_evidence_for_selected_scope --lib
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/platform-api/src/lib.rs
git commit -m "Constrain external temporary scope retrieval to selected documents"
```

## Task 4: Make Parse Status Model-Visible Inside Temporary Scope

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Relevant tests:
  - `assistant_run_supplies_document_parse_status_for_failed_and_reparsing_documents`
  - `assistant_run_provider_input_mentions_document_parse_status_supply`

**Step 1: Write or extend test**

Add:

```rust
#[tokio::test]
async fn assistant_run_external_temporary_scope_supplies_parse_status_for_range_documents()
```

Setup:
- Create two documents in the same canonical dataset.
- Mark one `failed` or `reparsing` through existing workflow/metadata fixtures.
- Build temporary selected scope with both documents.

Expected:
- Evidence state contains parse status supply for the affected document.
- Provider input includes concise parse status guidance.
- The model-facing context says this is for the current external document range.

**Step 2: Run test**

```bash
cargo test -p platform-api assistant_run_external_temporary_scope_supplies_parse_status_for_range_documents --lib
```

Expected: PASS if existing parse-status supply already keys off selected documents; otherwise FAIL.

**Step 3: Implement minimal fix if needed**

Update the document-parse-status supply builder to prefer `selected_document_ids_from_scope` over dataset-wide scanning when temporary scope is present.

**Step 4: Run focused tests**

```bash
cargo test -p platform-api assistant_run_external_temporary_scope_supplies_parse_status_for_range_documents --lib
cargo test -p platform-api assistant_run_supplies_document_parse_status_for_failed_and_reparsing_documents --lib
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/platform-api/src/lib.rs
git commit -m "Surface parse status for external temporary document scopes"
```

## Task 5: Keep External Chat Semantics Natural

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify if needed: `scripts/run-external-direct-reply-smoke.sh`

**Step 1: Write test**

Add:

```rust
#[tokio::test]
async fn external_channel_temporary_scope_keeps_direct_reply_contract()
```

Expected:
- External event with document range still creates normal AssistantRun request.
- `selected_scope` includes `temporary_dataset`.
- Final reply path remains provider-authored text.
- No orchestration acknowledgement is returned as final text.

**Step 2: Run focused tests**

```bash
cargo test -p platform-api external_channel_temporary_scope_keeps_direct_reply_contract --lib
cargo test -p platform-api assistant_run_provider_input_enforces_external_channel_direct_reply_contract --lib
```

Expected: PASS.

**Step 3: Extend smoke if valuable**

If the new test materially guards third-party behavior, add it to `scripts/run-document-understanding-smoke.sh` or `scripts/run-external-direct-reply-smoke.sh`.

**Step 4: Commit**

```bash
git add crates/platform-api/src/lib.rs scripts/run-document-understanding-smoke.sh scripts/run-external-direct-reply-smoke.sh
git commit -m "Guard direct replies for external temporary document scopes"
```

## Task 6: Document Third-Party Request Semantics

**Files:**
- Modify only after current dirty doc changes are understood:
  - `docs/integrations/third-party-document-parse-first-phase-sendable.zh-CN.md`
  - `docs/integrations/third-party-document-parse-first-phase-sendable.zh-CN.html`
  - `docs/integrations/pure-third-party-integration-guide.zh-CN.md`

**Step 1: Update API explanation**

Document that per-event document range is:

```json
{
  "available_document_source_id": "src-docs",
  "available_document_external_ids": ["doc-a", "doc-b"]
}
```

And means:
- V3 resolves those document IDs.
- V3 builds a conversation-scoped temporary dataset.
- The original documents remain in their canonical V3 dataset.
- The temporary dataset is only the current conversation/range boundary.

**Step 2: Update FAQ**

Add:
- “同一文档能否出现在多个数据集？”
- Answer: yes through temporary/virtual membership; canonical storage remains single owner in P0.

**Step 3: Commit separately**

```bash
git add docs/integrations
git commit -m "Document external temporary document scopes"
```

## Task 7: P0 Full Verification And Deploy

**Files:**
- No source edits unless tests expose issues.

**Step 1: Run local verification**

```bash
cargo fmt
cargo test -p platform-api external_channel_document_scope --lib
cargo test -p platform-api assistant_run_external_temporary_scope --lib
bash scripts/run-document-understanding-smoke.sh
bash scripts/run-external-direct-reply-smoke.sh
```

Expected: all PASS.

**Step 2: Push**

```bash
git push origin main
```

**Step 3: Deploy to 8 server**

```bash
ssh 8服务器 "cd /srv/aiv3/repo && git pull --ff-only && bash scripts/run-document-understanding-smoke.sh"
ssh 8服务器 "cd /srv/aiv3/repo && bash scripts/run-external-direct-reply-smoke.sh"
ssh 8服务器 "cd /srv/aiv3/repo && CC=clang CXX=clang++ cargo build --release -p platform-api"
ssh 8服务器 "sudo systemctl restart aiv3-platform-api.service"
ssh 8服务器 "systemctl is-active aiv3-platform-api.service aiv3-ingest-worker.service aiv3-retrieval-worker.service aiv3-assistant-run-worker.service"
```

Expected:
- Target HEAD is the new commit.
- Both smokes pass.
- Services are active.

## Task 8: P1 Schema Design For Durable Multi-Dataset Membership

**Files:**
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: storage and platform tests in existing test modules.

**Step 1: Write storage test first**

Add a storage test that proves:
- One `document_id` can be linked to two `dataset_id` values.
- One membership can expire.
- Canonical document `dataset_id` remains unchanged.

**Step 2: Add table**

Add to initial schema:

```sql
create table if not exists dataset_document_memberships (
  tenant_id uuid not null references tenants(id) on delete cascade,
  dataset_id uuid not null references datasets(id) on delete cascade,
  document_id uuid not null references documents(id) on delete cascade,
  membership_kind text not null default 'curated',
  source text not null default 'manual',
  expires_at timestamptz,
  created_at timestamptz not null default now(),
  primary key (tenant_id, dataset_id, document_id)
);
```

Add indexes:

```sql
create index if not exists dataset_document_memberships_document_idx
  on dataset_document_memberships (tenant_id, document_id);

create index if not exists dataset_document_memberships_expiry_idx
  on dataset_document_memberships (tenant_id, expires_at)
  where expires_at is not null;
```

**Step 3: Add repository methods**

Add methods:

```rust
create_or_update(...)
list_document_ids_by_dataset(...)
list_dataset_ids_by_document(...)
delete_expired(...)
```

**Step 4: Run storage tests**

```bash
cargo test -p storage dataset_document_membership --lib
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/storage/src/lib.rs
git commit -m "Add dataset document membership storage"
```

## Task 9: P1 Read Path Integration

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify if needed: `crates/storage/src/lib.rs`

**Step 1: Write platform test**

Add:

```rust
#[tokio::test]
async fn dataset_document_memberships_allow_document_in_multiple_dataset_scopes()
```

Expected:
- Dataset A is canonical owner.
- Dataset B has membership to same document.
- Dataset B retrieval/listing can include the document.
- Dataset A remains intact.

**Step 2: Update read helpers**

Update helpers that currently only use `documents.list_by_dataset` when the caller explicitly needs dataset content:
- visible document IDs for dataset
- document list/detail search paths
- retrieval evidence filtering

Do not change ingest ownership.

**Step 3: Run tests**

```bash
cargo test -p platform-api dataset_document_memberships --lib
cargo test -p platform-api search_dataset_retrieval_returns_ranked_hits_with_document_ids --lib
```

Expected: PASS.

**Step 4: Commit**

```bash
git add crates/platform-api/src/lib.rs crates/storage/src/lib.rs
git commit -m "Read dataset documents through membership"
```

## Task 10: P1 Temporary Dataset Lifecycle

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`

**Step 1: Write lifecycle test**

Add:

```rust
#[tokio::test]
async fn external_channel_temporary_dataset_memberships_expire_without_moving_documents()
```

Expected:
- External event creates or reuses a temporary dataset.
- Membership rows get an `expires_at`.
- Expiry cleanup removes membership rows or archives temporary dataset.
- Canonical document remains queryable through original dataset.

**Step 2: Implement minimal TTL**

Start with a deterministic helper called during external event ingestion:

```rust
cleanup_expired_temporary_dataset_memberships(state).await?;
```

Avoid adding a new worker unless cleanup cost becomes a problem.

**Step 3: Run tests**

```bash
cargo test -p platform-api external_channel_temporary_dataset_memberships_expire_without_moving_documents --lib
```

Expected: PASS.

**Step 4: Commit**

```bash
git add crates/platform-api/src/lib.rs crates/storage/src/lib.rs
git commit -m "Expire external temporary dataset memberships"
```

## Rollback Plan

- P0 rollback: remove temporary scope enrichment only; no data migration needed.
- P1 rollback before deploy: revert membership commits.
- P1 rollback after deploy: stop using membership read paths first, keep table harmlessly unused, then remove after backup/migration review.

## Done Criteria

- Third-party can send a per-message document range.
- V3 answers only from that range.
- The model sees unresolved/failed/reparsing document status instead of silent failure.
- The original dataset/document ownership does not change.
- Same document can participate in multiple formal scopes after P1.
- 8 server document-understanding and external-direct-reply smokes pass.

