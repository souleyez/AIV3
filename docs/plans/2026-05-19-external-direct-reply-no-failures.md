# V3 External Direct Reply No Failures Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make V3 external ordinary chat always return a model-authored direct answer instead of orchestration, accepted, task-status, or provider-failure copy.

**Architecture:** Split ordinary chat from action/status workflows with an explicit strict-direct-reply contract. Provider errors, timeouts, empty outputs, and suppressed outputs become retryable completion failures that enter a bounded fallback chain before any response is returned. Parsing quality failures are handled before retrieval by routing low-quality PDF extraction through a document VLM/OCR fallback lane.

**Tech Stack:** Rust `platform-api`, Rust `llm-gateway`, Next.js proxy routes, existing external-channel tests, shell smoke scripts, MiniMax/OpenAI-compatible provider routing.

---

### Task 1: Lock The Ordinary External Reply Contract

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`
- Optional docs update later: `docs/integrations/*.md`

**Steps:**
1. Add failing tests for ordinary external chat asserting:
   - `reply.reply_type == "text"`
   - `reply.task_status == "answered"`
   - `reply.text` is present and does not contain fixed accepted copy.
   - Generic prompts like `status`, `查询状态`, `1+1等于几` do not produce `TaskStatus`, `RequiresConfirmation`, `accepted`, `model_unavailable`, or `model_output_suppressed`.
2. Run targeted tests and confirm failures where current placeholder/provider-failure branches remain.
3. Introduce a helper such as `external_channel_is_strict_direct_reply_message(...)` or equivalent guard around ordinary chat.
4. Keep action/search/status-specific branches only for explicit action intent; do not let vague ordinary language enter them.
5. Run `cargo test -p platform-api external_channel --lib`.
6. Commit this task separately.

**Acceptance:**
- Ordinary third-party chat cannot return `accepted` or task-status payloads from the normal chat path.

### Task 2: Remove Production Accepted Placeholder From Ordinary Chat

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify if needed: `crates/llm-gateway/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**
1. Add a failing test for `chat_runtime.mode == "placeholder"` under external ordinary chat.
2. Replace `external_channel_chat_acceptance_reply(...)` fallback in ordinary external chat with a retry/fallback completion path.
3. For production strict mode, treat missing provider route as readiness/configuration failure, not a user-visible accepted message.
4. Preserve placeholder behavior only in tests or explicit non-production fixture paths if existing coverage needs it.
5. Run `cargo test -p platform-api external_channel --lib`.
6. Commit.

**Acceptance:**
- No ordinary external response path can emit the fixed “我已收到...” / “已收到指令...” copy.

### Task 3: Add Provider Timeout And Retry/Fallback Chain

**Files:**
- Modify: `crates/llm-gateway/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/llm-gateway/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**
1. Add failing `llm-gateway` tests proving OpenAI-compatible provider applies profile/env timeout.
2. Wire `ModelProviderProfile.timeout_ms` into `OpenAiCompatibleLlmProvider::new(...)` and request execution.
3. Add platform tests where provider returns timeout/http 5xx/empty/suppressed/finish_reason error and the external chat path retries.
4. Implement a bounded completion helper for ordinary chat:
   - primary provider attempt;
   - retry on timeout, 429, 5xx, empty text, suppressed output, or `runtime.provider_failure`;
   - fallback route/profile when configured;
   - final successful model text only.
5. Set default budgets conservatively:
   - per attempt around 90-120 seconds;
   - total external sync budget below public proxy timeout;
   - leave Nginx 300s as outer ceiling.
6. Run:
   - `cargo test -p llm-gateway`
   - `cargo test -p platform-api external_channel --lib`
7. Commit.

**Acceptance:**
- MiniMax timeout or provider failure is not returned to the third party as answer text; it triggers another model attempt first.

### Task 4: Fix SSE And Duplicate Idempotency Semantics

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify if needed: `apps/web/app/lib/platform-api.js`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**
1. Add tests for `/events/stream` ordinary chat:
   - user-visible `delta/completed` contains model answer only;
   - no user-visible accepted copy.
2. Add duplicate-idempotency tests:
   - if original run completed, duplicate returns/replays final answer;
   - if original run is running, response is transport pending only and not documented as assistant answer.
3. Adjust SSE event naming or payload comments so `accepted` is transport-only, or omit it for strict-direct ordinary chat.
4. Update duplicate branch currently returning `duplicate_accepted` to look up completed run output first.
5. Run `cargo test -p platform-api external_channel --lib`.
6. Commit.

**Acceptance:**
- Third-party retry cannot surface `duplicate_accepted` as a user answer.

### Task 5: Add PDF Parse Quality Gate And VLM/OCR Fallback

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify or add worker code where upload ingest parse quality is finalized.
- Test: `crates/platform-api/src/lib.rs`
- Possibly use existing `MODEL_LANE_DOCUMENT_VLM` in `crates/llm-gateway/src/lib.rs`.

**Steps:**
1. Add tests for a PDF ingest result with extremely short text, for example one extracted character.
2. Define parse quality signals:
   - extracted text length too short;
   - page count present but text coverage near zero;
   - image-heavy PDF indicators when available;
   - parser error or empty chunks.
3. When quality is below threshold, enqueue or call the document VLM/OCR fallback lane before making the document visible as successfully parsed.
4. Record parse metadata:
   - `parse_quality.status`;
   - `fallback_provider`;
   - `fallback_model`;
   - `fallback_status`;
   - text/chunk counts before and after fallback.
5. Ensure retrieval/detail endpoints prefer fallback text chunks when available.
6. Run targeted document parse tests and existing document/media tests.
7. Commit.

**Acceptance:**
- A PDF that only produced one character is not treated as a successful text parse; VLM/OCR fallback is attempted and visible in diagnostics.

### Task 6: Update Third-Party Docs And Smoke Scripts

**Files:**
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Modify: `docs/integrations/third-party-integration-api.md`
- Modify or create: `scripts/run-external-direct-reply-smoke.sh`
- Modify existing smoke scripts if better.

**Steps:**
1. Update docs so ordinary chat examples show `task_status=answered` and `reply_type=text`.
2. Clearly mark `accepted` as non-answer transport state where still applicable.
3. Add smoke script assertions:
   - ordinary math question returns model answer;
   - `status` and `查询状态` return model text, not artifact status;
   - response text does not contain fixed accepted strings;
   - response status is not `model_unavailable`, `model_output_suppressed`, `duplicate_accepted`, or `accepted`.
4. Add an optional streaming smoke that consumes SSE through completion.
5. Run docs-related tests if present and the new smoke locally against dev.
6. Commit.

**Acceptance:**
- Regression smoke catches the exact failure the third party saw.

### Task 7: Deploy And Online Verification

**Files:**
- No source edits unless deployment docs need adjustment.

**Steps:**
1. Run full local verification:
   - `cargo fmt --all --check`
   - `cargo test -p llm-gateway`
   - `cargo test -p platform-api external_channel --lib`
   - document parse targeted tests
   - external direct reply smoke
   - relevant web build if proxy/docs UI changed
2. Deploy to 8 server.
3. Confirm service env has provider mode, model, and timeout values.
4. Run public smoke through `https://v3.elepcloud.com`:
   - `1+1等于几？请直接回答。`
   - `status`
   - `查询状态`
   - a normal Chinese customer question.
5. Check event trail for:
   - model completion event;
   - no accepted final answer;
   - fallback event only when provider actually failed.
6. Commit any deployment notes if needed.

**Acceptance:**
- Online public endpoint no longer reproduces fixed accepted/received copy for ordinary third-party chat.

---

## Priority

1. Task 1, 2, 3: must land first. These close the visible third-party ordinary-chat failure.
2. Task 4: close retry/SSE edge cases immediately after, because customers often retry on timeout.
3. Task 5: close PDF one-character parse quality; this is separate from chat but affects answer truthfulness.
4. Task 6, 7: lock docs, smoke, and online verification so the regression stays closed.

## Deployment Notes

- Public Nginx already allows long proxy reads at about 300 seconds, so the first timeout fix belongs in the provider client and request budget.
- External action callback timeout around 12 seconds is a separate action-path issue, not the ordinary chat bottleneck.
- `EXTERNAL_DOCUMENT_PARSE_TIMEOUT_SECS` affects document download/parse ingestion and should be reviewed for large PDFs, but it does not replace parse quality fallback.
