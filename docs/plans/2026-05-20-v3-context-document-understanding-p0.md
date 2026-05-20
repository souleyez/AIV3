# V3 Context And Document Understanding P0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task.

**Goal:** Close the remaining V3 P0 gaps around third-party document scope, direct replies, async parsing, document understanding, and global intent-gated user context.

**Architecture:** Treat V3 context as explicit, typed scopes: selected document datasets, temporary external document scopes, global hidden user-context scopes, current conversation history, parser status, and artifact state. The model should receive only the scopes selected by the planner or explicitly requested by the user; user history and cross-conversation context must be intent-gated. Parsing should become asynchronous and quality-aware, with PaddleOCR as the preferred structured parser when available and MiniMax VLM as the low-quality fallback.

**Tech Stack:** Rust `platform-api`, `assistant-runtime`, `storage`, `ingest-worker`, PostgreSQL, PaddleOCR PP-StructureV3 sidecar, MiniMax VLM lane, shell smoke scripts, 8 server deployment.

---

## Baseline

- Current active deployment target: 8 server.
- Do not sync or deploy 120 server after the 120 cutoff.
- Current deployed baseline was `b9a6a85` when this plan was written.
- External temporary document memberships are already implemented and deployed:
  - `dataset_document_memberships`
  - selected external document allowlist
  - retrieval/evidence/entity-scan visibility through memberships
- Existing global behavior already has `conversation_memory_items` and scope-planner intent gating, but there is no explicit global "user context dataset" abstraction.

## Product Rules

1. Third-party ordinary chat must return a model-authored answer, not orchestration copy, accepted copy, or the old fixed "已收到指令..." fallback.
2. Third-party document ids are a scope request. V3 must bind them to a dataset/scope on our side before answer-time retrieval.
3. Current conversation history may remain short-turn continuity. Cross-conversation user history is a hidden user-context dataset and must not be supplied by default.
4. If global user-context dataset support does not already exist, build it as a global V3 capability. Third-party `sender_external_id` becomes one identity source for that global capability.
5. Document parsing must not report low-quality extraction as success. One-character PDFs, empty chunks, and obvious OCR misses must trigger fallback or explicit parser status supply.
6. Parsing must not block normal upload, classification, or third-party reply flow longer than the public sync budget.
7. Parser status, retrying state, failed state, and available partial context must be supplied to the model so it can answer honestly.
8. Document understanding should move from hard chunking toward sections, paragraphs, entities, and noun terms.

## Active Gaps

- **External user context:** `sender_external_id` is present, but same-user history across third-party conversations is not persisted as a selectable global user-context scope.
- **Global user dataset:** there is no first-class product abstraction for a hidden user-context dataset; only thread-scoped `conversation_memory_items`.
- **External temp dataset strictness:** selected scope still keeps canonical dataset ids in some places while temporary dataset metadata is nested.
- **Document list/detail visibility:** `/v1/documents` and `/v1/documents/{id}` still need membership-aware access for temporary document scopes.
- **Third-party parse request lifecycle:** when a third party requests parsing, V3 must create/resolve the corresponding dataset/scope and not rely on the third party to infer internal ids.
- **Async parse:** upload should return quickly after basic classification; detailed parse/OCR/VLM should continue asynchronously.
- **Parse quality:** one-character or empty extraction must trigger PaddleOCR/MiniMax fallback and status metadata.
- **Document understanding:** paragraph segmentation, section structure, entity scan, and noun-term extraction need to feed retrieval and planner hints.
- **Direct reply reliability:** provider timeout/empty/suppressed output still needs bounded retries and no fake assistant success state.
- **Static HTML closure:** quick static page generation needs a reliable HTML download/export state loop.

## Phase 0: Freeze Baseline And Tests

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Modify: `scripts/run-external-direct-reply-smoke.sh`
- Modify: `scripts/run-document-understanding-smoke.sh`

**Step 1: Add failing tests for the remaining gaps**

Add tests before implementation for:

- external selected document ids create/resolve an internal temporary dataset/scope;
- membership-backed documents are visible to detail/list APIs when selected by external scope;
- ordinary external chat cannot emit fixed accepted/fallback text;
- user-context memory is available only when a history-intent prompt asks for it;
- one-character PDF parse is not marked as successful extraction.

**Step 2: Run the focused failing set**

Run:

```powershell
cargo test -p assistant-runtime conversation_memory --lib
cargo test -p platform-api external_channel --lib
cargo test -p platform-api dataset_document_membership --lib
```

Expected:

- Existing implemented tests pass.
- New tests fail for the exact missing behavior.

**Step 3: Commit only tests if useful**

Commit message:

```text
test: capture v3 context and parse p0 gaps
```

## Phase 1: Global User Context Dataset

**Files:**

- Modify: `crates/assistant-runtime/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs` only if existing memory APIs cannot support the adapter.

**Design:**

Create a global internal abstraction named "user context dataset" or "user context scope". Phase 1 should be backed by `conversation_memory_items`; do not create normal visible document datasets for chat history unless the storage shape proves necessary.

Global identities:

```text
user:{tenant_id}:{user_id}
local-user:{tenant_id}:{local_thread_or_secret_fingerprint}
external-user:{platform}:{tenant_external_id}:{bot_external_id}:{sender_external_id}
```

The external key is one identity provider for the same global concept.

**Step 1: Extend scope candidate semantics**

Keep `ScopeCandidateType::ConversationMemory`, but make ids explicit:

```text
local-thread:{local_thread_id}
user-context:{context_key}
external-user:{platform}:{tenant_external_id}:{bot_external_id}:{sender_external_id}
```

The selected scope should still use `conversation_memory`, but evidence supply must understand these ids.

**Step 2: Keep the global gate**

Reuse:

```rust
conversation_memory_available && prompt_references_conversation_history(prompt)
```

Add tests showing:

- generic prompt: no user-context memory in selected scope;
- history prompt: user-context memory selected;
- selected scope uses `historyPolicy = intent_gated_selected` only when memory was selected.

**Step 3: Add global evidence loader**

Add a helper similar to:

```rust
async fn load_selected_conversation_memory_items(
    state: &AppState,
    selected_scope: &Value,
    current_local_thread_id: Option<&str>,
    current_user_id: Option<UserId>,
) -> Result<Vec<Value>, ApiError>
```

It must:

- load current local-thread memory only when selected;
- load user-context memory only when selected;
- preserve `owner_user_id_is_visible`;
- ignore malformed or unrecognized memory ids.

**Step 4: Validate**

Run:

```powershell
cargo test -p assistant-runtime conversation_memory --lib
cargo test -p platform-api conversation_memory --lib
```

## Phase 2: Third-Party External User Context

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`

**Step 1: Add external user key helpers**

Near the current external local-thread helper, add:

```rust
fn external_user_context_key(
    platform: &str,
    tenant_external_id: &str,
    bot_external_id: &str,
    sender_external_id: &str,
) -> Option<String>
```

Return `None` if any required component is blank.

**Step 2: Persist completed turn memory**

After a successful external assistant reply, write a compact memory item under the external user-context key.

Metadata should include:

- channel connection id;
- conversation external id;
- message external id;
- source assistant run id;
- sender external id;
- source local thread id.

Do not persist memory for provider failures, pending replies, fixed fallback copy, or empty output.

**Step 3: Expose candidate only when memory exists**

When building an external request:

- check if the external user-context key has memory items;
- add a user-context candidate only if memory exists;
- do not append this memory to `messages`;
- keep `mention_external_user_ids` out of this feature.

**Step 4: Validate isolation**

Tests:

- same `sender_external_id`, different `conversation_external_id`: can recall only when history intent is present;
- different `sender_external_id`: cannot see another user's memory;
- different tenant/bot: isolated;
- generic prompt from same user does not use cross-conversation memory.

Run:

```powershell
cargo test -p platform-api external_user_memory --lib
```

## Phase 3: External Temporary Document Dataset Closure

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`

**Step 1: Make temporary scope the explicit selected dataset**

For external requests with `available_document_external_ids`:

- create or refresh a temporary dataset/scope id;
- put that id in the active selected scope;
- keep canonical dataset/document provenance in metadata;
- keep document allowlist in `documents`.

**Step 2: Make document APIs membership-aware**

Update list/detail helpers so temporary membership can authorize:

- `/v1/documents`
- `/v1/documents/{id}`
- any document detail/read tool path used by ReAct.

**Step 3: Cleanup lifecycle**

Keep membership TTL cleanup. Add temporary dataset cleanup/archival if current temp dataset rows can accumulate indefinitely.

**Step 4: Validate**

Run:

```powershell
cargo test -p platform-api dataset_document_membership --lib
cargo test -p platform-api external_channel --lib
```

## Phase 4: Third-Party Parse Request Creates Internal Dataset

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`

**Step 1: Resolve dataset on parse request**

When a third party requests document parsing:

- resolve `available_document_source_id` or equivalent source key;
- create a V3 internal dataset if one does not exist;
- attach parsed documents to that dataset;
- return both third-party ids and V3 internal ids in status/detail responses.

**Step 2: Preserve repeated request idempotency**

Use existing idempotency key/message external id rules so repeated parse requests do not create duplicate datasets/documents.

**Step 3: Validate**

Tests:

- parse request with only external document ids creates internal dataset binding;
- repeated parse request reuses dataset/document;
- answer request immediately after parse sees pending parse status for the same scope.

Run:

```powershell
cargo test -p platform-api external_document_parse --lib
```

## Phase 5: Async Parse Lifecycle And Model Supply

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/migrations/*.sql` if current metadata cannot represent parse attempts cleanly.

**Step 1: Split fast classify from detailed parse**

Upload/parse request should quickly persist:

- document shell;
- external id mapping;
- initial mime/type classification;
- parse lifecycle state: `queued`, `running`, `partial`, `retrying`, `failed`, `succeeded`.

Detailed extraction continues in worker.

**Step 2: Supply parse state to model**

Extend evidence state to include parse status for selected documents:

- which files are ready;
- which files are still parsing;
- which files failed;
- whether retry/VLM fallback is in progress;
- whether answer should be partial.

**Step 3: Validate timeout budgets**

External sync reply path must stay under public proxy timeout. Long parsing must return a status-aware answer, not block until timeout.

Run:

```powershell
cargo test -p platform-api document_parse_status_supply --lib
cargo test -p ingest-worker parse_lifecycle --lib
```

## Phase 6: PaddleOCR Primary Parser And MiniMax VLM Fallback

**Files:**

- Modify: `crates/ingest-worker/src/main.rs`
- Modify: parser helper module if already split out.
- Modify: `docs/validation/ingest-runtime-dependency-gate.md`
- Modify: `scripts/run-document-understanding-smoke.sh`

**Step 1: Prefer PaddleOCR when configured**

Use PaddleOCR PP-StructureV3 for PDF/image documents when the runtime is available.

Suggested env contract:

```text
DOCUMENT_PDF_PARSE_ENGINE=paddleocr_first
DOCUMENT_PADDLEOCR_ENABLED=true
DOCUMENT_PADDLEOCR_TIMEOUT_SECS=...
DOCUMENT_PADDLEOCR_MAX_PAGES=...
```

If PaddleOCR is not configured, current parser remains safe.

**Step 2: Add quality gate**

Reject parser output as "successful text extraction" when:

- text length is below threshold;
- output is one character or mostly punctuation;
- no chunks/paragraphs were produced;
- extracted language/structure is obviously broken;
- parser error is present.

**Step 3: Trigger MiniMax VLM fallback**

When PaddleOCR/native output fails quality gate:

- call MiniMax VLM document lane if configured;
- record fallback attempt, timeout, and result;
- never silently convert bad output into trusted chunks.

**Step 4: Make PaddleOCR default after smoke**

If local and 8 smoke prove good quality, set `paddleocr_first` as default for supported PDF/image parse runtime on 8.

**Step 5: Validate**

Run:

```powershell
cargo test -p ingest-worker pdf_parse_quality --lib
bash scripts/run-document-understanding-smoke.sh
```

## Phase 7: Paragraphs, Sections, Entities, And Noun Terms

**Files:**

- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Add migration only when metadata is no longer enough.

**Step 1: Store document structure**

Persist parser structure in metadata first:

- blocks;
- headings;
- paragraph ranges;
- tables;
- OCR confidence;
- source parser.

**Step 2: Chunk by section and paragraph**

Update chunk construction to prefer:

- section title + paragraph text;
- table as table block;
- no hard fixed-size split unless no structure exists.

**Step 3: Generalize entity scan**

Extend beyond company names:

- organization/company;
- person;
- product/project;
- place;
- domain-specific noun phrases.

**Step 4: Feed planner hints**

Populate dataset metadata:

- `document_title_hints`;
- `section_title_hints`;
- `noun_term_hints`;
- `material_hints`;
- `parse_status_summary`.

**Step 5: Validate**

Run:

```powershell
cargo test -p platform-api dataset_entity_scan --lib
cargo test -p assistant-runtime scope_planner --lib
```

## Phase 8: External Direct Reply Reliability

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/llm-gateway/src/lib.rs` if provider timeout policy needs lane support.
- Modify: `scripts/run-external-direct-reply-smoke.sh`

**Step 1: Strict direct-reply guard**

External ordinary text chat must reject or retry:

- empty output;
- fixed accepted copy;
- orchestration/status copy;
- suppressed output;
- provider timeout/5xx if retry budget remains.

**Step 2: Bounded fallback chain**

Use fallback providers/lanes inside a total external sync budget. If all fail, return a transparent error/status payload, not a fake assistant answer.

**Step 3: Validate**

Run:

```powershell
cargo test -p platform-api external_channel --lib
bash scripts/run-external-direct-reply-smoke.sh
```

## Phase 9: Static HTML Download Closure

**Files:**

- Modify: static-page route/components already used by the quick effect preview entry.
- Modify: `scripts` smoke if one exists for static-page generation.

**Step 1: Ensure generated HTML has a stable artifact state**

The quick HTML path should expose:

- generation id;
- preview URL;
- download URL;
- failed/running/succeeded status;
- retryable error reason.

**Step 2: Validate download loop**

Smoke:

- create quick static page;
- wait for generated HTML;
- download HTML;
- verify non-empty HTML and referenced assets.

## Phase 10: Rollout And Smoke On 8

**Files:**

- Modify: deployment notes if needed.

**Step 1: Local validation**

Run:

```powershell
cargo test -p assistant-runtime conversation_memory --lib
cargo test -p platform-api external_channel --lib
cargo test -p platform-api dataset_document_membership --lib
cargo test -p ingest-worker pdf_parse_quality --lib
bash scripts/run-external-direct-reply-smoke.sh
bash scripts/run-document-understanding-smoke.sh
```

**Step 2: Deploy only to 8**

Do not deploy or sync 120.

On 8:

```bash
cd /srv/aiv3/repo
git pull --ff-only
cargo test -p platform-api external_channel --lib
bash scripts/run-external-direct-reply-smoke.sh
bash scripts/run-document-understanding-smoke.sh
```

**Step 3: Service validation**

Check:

- `aiv3-platform-api.service`
- `aiv3-ingest-worker.service`
- `aiv3-retrieval-worker.service`
- `aiv3-assistant-run-worker.service`

**Step 4: Third-party replay smoke**

Replay:

- parse request with external ids;
- answer request with document scope;
- same sender history-intent prompt;
- different sender isolation prompt;
- ordinary prompt no user-history prompt;
- static HTML generation/download path.

## Definition Of Done

- Third-party document ids always resolve to an internal V3 scope/dataset before retrieval.
- Temporary external document scopes are honored by retrieval, detail reads, list reads, parse status supply, and entity scan.
- User history exists as a global hidden user-context dataset/scope, not a third-party-only special case.
- User history is not supplied by default and only appears when the planner selects conversation memory for historical intent.
- `mention_external_user_ids` never grants access to mentioned users' histories.
- Bad parser output is not marked successful; PaddleOCR/MiniMax fallback state is visible.
- Parser lifecycle status is supplied to model answers.
- Paragraph/section/noun/entity hints improve retrieval and scope planning.
- External ordinary chat cannot return fixed "已收到指令..." style fallback copy.
- 8 server smoke passes; 120 remains untouched.

## Supersedes / References

This is the active combined execution plan. It incorporates the remaining work from the May 19-20 external temporary-dataset, direct-reply, PaddleOCR document-understanding, and external-user history-memory slices. The older slice plans were removed after consolidation.
