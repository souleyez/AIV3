# DataMax Streaming Session Upgrade Implementation Plan

**Goal:** Upgrade DataMax external and main-site streaming sessions so users can see meaningful progress, receive token-level answers where possible, reconnect safely after interruptions, and always get a final answer, follow-up question, or artifact link without exposing internal generation details.

**Architecture:** Keep `/v1/external/channels/{connection_id}/events`, `/events/stream`, and `/assistant-runs/{assistant_run_id}/reply` as the public surface, but make them share one sanitized response/status contract. Persist stream events on `assistant_run_events` so short SSE connections, reconnects, and polling all read from the same event timeline. Add real model token streaming for normal Q&A while long-running artifact/database/static-page work continues to emit phase events and `status_url` updates.

**Tech Stack:** Rust, Axum SSE, Tokio streams, PostgreSQL `assistant_run_events`, existing `ExternalChannelEventResponse`, existing assistant run storage, Next.js main site chat UI, third-party integration docs.

---

## Current State

- External SSE endpoint exists at `POST /v1/external/channels/{connection_id}/events/stream`.
- JSON endpoint exists at `POST /v1/external/channels/{connection_id}/events`.
- Polling endpoint exists at `GET /v1/external/channels/{connection_id}/assistant-runs/{assistant_run_id}/reply`.
- Static page/report generation already emits sanitized external events:
  - `external_channel.static_page_planning`
  - `external_channel.static_page_queued`
  - `external_channel.static_page_preview_ready`
  - `external_channel.static_page_publish_progress`
  - `external_channel.static_page_published`
  - `external_channel.static_page_issue`
  - `external_channel.static_page_continue_polling`
- External docs no longer expose internal high-quality visualization implementation details.
- Long static-page jobs can continue through `status_url` polling after an SSE timeout.
- 2026-06-02 update: `/events/stream` structured external events now use `schema=v3.external_channel.sse.v1`, public stream events are persisted into `assistant_run_events`, and reconnect can replay persisted public events by `Last-Event-ID`, `since_sequence`, or `stream_since_sequence`.
- 2026-06-02 update: normal Q&A live delta streaming is wired behind `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED=true`; provider deltas are buffered per attempt until the reply passes display-quality checks, so rejected generic fallback text is not leaked to third-party clients. Retry/timeout/rejection paths emit public `external_channel.answer_retrying` progress with `status_url`.
- 2026-06-06 update: main-site `POST /v1/assistant-runs/{run_id}/continue/stream` now uses the same live answer worker/channel as `POST /v1/assistant-runs/stream` when `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true`. Continue completion skips the final full-text delta if live deltas were already emitted, so the browser does not duplicate answer text.

## Main Problems To Solve

1. Normal Q&A live streaming is available only behind a default-off feature flag; production rollout still needs smoke on 8 server with the active model pool.
2. Native provider token deltas are emitted only after per-attempt quality checks; this avoids leaking rejected answers but means the first rollout favors safety over raw token immediacy.
3. Reconnect support covers persisted public progress events; transient live answer deltas are not yet replayed from storage.
4. `/events`, `/events/stream`, and `/assistant-runs/{id}/reply` can drift in status shape and field sanitization.
5. Long tasks must not become false failures when they merely exceed an SSE timeout.
6. "Need more information" must be an executable state, not a dead-end answer.
7. Artifact links should stream immediately once available, before unrelated background work finishes.
8. If a static page response already contains a valid generated-artifact URL, public exits should treat it as published even if an older internal status still says running.

## Public Contract Principles

- Do not expose model provider names, image-generation details, executor details, internal task template names, raw prompt, raw diff, raw database URL, credentials, or private logs.
- Every SSE event must be safe for third-party UI display or logging after normal JSON escaping.
- Every user-visible terminal path must produce one of:
  - final text answer,
  - final artifact link,
  - clear follow-up question with resumable state,
  - processing state with `status_url`,
  - actionable issue state with retry/poll/manual handling guidance.
- `failed` is reserved for non-continuable states. Slow, retrying, queued, waiting, and background-running states remain `processing`.
- `status_url` is authoritative for reconnect and short-connection clients.

## Target SSE Envelope V1

Every public event should have this common shape:

```json
{
  "schema": "v3.external_channel.sse.v1",
  "event_id": "run-id:000012",
  "sequence": 12,
  "assistant_run_id": "run-id",
  "idempotency_key": "third-party:tenant:msg",
  "conversation_external_id": "conv-001",
  "phase": "retrieval|answering|artifact|database|static_page|completed|issue",
  "status": "processing|completed|needs_input|retrying|failed",
  "display_text": "DataMax 正在扩展检索范围...",
  "status_url": "https://v3.elepcloud.com/v1/external/channels/.../assistant-runs/.../reply",
  "poll_after_seconds": 15,
  "data": {}
}
```

Event names remain human-readable and stable:

```text
external_channel.started
external_channel.retrieval_started
external_channel.retrieval_expanded
external_channel.answer_delta
external_channel.answer_retrying
external_channel.answer_completed
external_channel.needs_input
external_channel.static_page_planning
external_channel.static_page_queued
external_channel.static_page_preview_ready
external_channel.static_page_publish_progress
external_channel.static_page_published
external_channel.issue
external_channel.continue_polling
external_channel.completed
external_channel.heartbeat
done
error
```

## P0 Plan

### Task 1: Define Stream Event Envelope

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add a contract struct for public SSE event envelope, or a local helper if contracts should stay lean:
   - `schema`
   - `event_id`
   - `sequence`
   - `assistant_run_id`
   - `idempotency_key`
   - `conversation_external_id`
   - `phase`
   - `status`
   - `display_text`
   - `status_url`
   - `poll_after_seconds`
   - `data`

2. Add helper:

```rust
fn external_channel_sse_envelope(
    run_id: Option<AssistantRunId>,
    idempotency_key: &str,
    conversation_external_id: &str,
    sequence: i64,
    phase: &str,
    status: &str,
    display_text: &str,
    status_url: Option<String>,
    poll_after_seconds: Option<u64>,
    data: Value,
) -> Value
```

3. Update `external_channel_static_page_sse_progress_events`, `external_channel_static_page_sse_preview_ready_events`, `external_channel_static_page_sse_prompt_events`, and `external_channel_static_page_sse_continue_polling_event` to wrap their payloads in the envelope while preserving existing data under `data`.

4. Keep existing top-level fields for one compatibility release if needed, but mark the envelope as preferred in docs.

5. Tests:
   - `external_channel_sse_envelope_has_sequence_and_schema`
   - `external_channel_static_page_sse_events_use_public_envelope`

**Commands:**

```powershell
cargo test -p platform-api external_channel_sse --lib
cargo test -p platform-api external_channel_static_page --lib
```

### Task 2: Persist Public Stream Timeline

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Possible migration: `migrations/*` only if existing `assistant_run_events.sequence_no` cannot be reused
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Reuse `assistant_run_events.sequence_no` as the stable stream sequence.
2. Add helper to append public stream events:

```rust
async fn append_external_channel_public_stream_event(
    state: &AppState,
    run_id: AssistantRunId,
    event_name: &str,
    envelope: Value,
    now: DateTime<Utc>,
) -> Result<(), ApiError>
```

3. When emitting SSE for long-running static page/report phases, append the same public envelope to run events.
4. Ensure sensitive internal events continue to exist for ops, but public replay only reads public stream events.
5. Tests:
   - public event sequence increments.
   - public replay excludes internal event payloads.

**Commands:**

```powershell
cargo test -p platform-api external_channel_public_stream --lib
```

### Task 3: Add Reconnect Support

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify docs: `docs/integrations/third-party-integration-api.zh-CN.md`
- Test: `crates/platform-api/src/lib.rs`

**API Behavior:**

- Support `Last-Event-ID` header.
- Support query param `since_sequence`.
- Support optional request body field `stream_since_sequence` for clients that cannot set headers.
- On reconnect, emit already persisted public stream events after the given sequence, then continue following the live run if it is still active.

**Steps:**

1. Parse:

```text
Last-Event-ID: <assistant_run_id>:<sequence>
?since_sequence=12
```

2. Add helper:

```rust
fn parse_external_channel_stream_resume(headers: &HeaderMap, query: &QueryMap, payload: &Value) -> Option<i64>
```

3. Add query-capable route extractor for `/events/stream`.
4. Before starting new processing, if idempotency key already maps to a run, replay public stream events after `since_sequence`.
5. If run is terminal, end with `completed` and `done`.
6. Tests:
   - reconnect replays only unseen events.
   - duplicate idempotency key with stream returns replay instead of starting a new run.
   - invalid `Last-Event-ID` does not panic and falls back to normal stream.

**Commands:**

```powershell
cargo test -p platform-api external_channel_stream_resume --lib
```

### Task 4: Token Stream Normal Q&A

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Inspect/modify: `crates/llm-gateway/src/lib.rs`
- Inspect/modify: `crates/assistant-runtime/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. Identify whether the active model gateway supports streaming responses.
2. Add an internal streaming path for normal chat only:
   - retrieval/context building first,
   - emit `retrieval_started`,
   - emit `retrieval_expanded` if quality gate expands supply,
   - emit `answer_delta` chunks,
   - emit `answer_completed`.

3. If the model gateway does not support streaming for a provider, emit phase progress and then a final completed answer. This is a graceful degradation, not an error.

4. Ensure the final answer is still attached to `assistant_runs.output_artifacts` and returned by `/reply`.

5. Tests:
   - streaming model path emits more than one `answer_delta`.
   - non-streaming model path emits progress then final answer.
   - no system fallback text is emitted as a fake answer.

**Commands:**

```powershell
cargo test -p platform-api external_channel_streaming_answer --lib
cargo test -p llm-gateway --lib
```

**2026-06-02 status:** Initial fallback progress completed. External `/events/stream` emits `external_channel.retrieval_started` immediately after `external_channel.started`, so third-party clients can show visible progress while DataMax prepares visible documents, data sources, and conversation context. This was later superseded by the gateway streaming contract and platform live SSE wiring below. Verified the fallback progress path with `cargo test -p platform-api generic_chat_page_event_stream_does_not_emit_accepted_as_answer_state --lib`, `cargo check -p platform-api`, and third-party docs checks.

**2026-06-02 status:** Gateway streaming contract completed locally. `llm-gateway::LlmProvider` now exposes `complete_streaming`, with a default buffered fallback for providers that do not support native streaming and an OpenAI-compatible SSE implementation that sends `stream=true`, parses `data:` chunks, returns usage/finish metadata, and calls back per delta. Platform live SSE wiring remains the next batch because the external ordinary-chat path still needs a background task/channel bridge to preserve model-pool retry, answer rejection, artifact persistence, and user-memory side effects. Verified with `cargo test -p llm-gateway --lib` and `cargo check -p platform-api`.

**2026-06-02 status:** Platform live SSE wiring completed locally behind the default-off `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED=true` flag. When enabled for `/v1/external/channels/{connection_id}/events/stream`, DataMax runs the existing external ordinary-chat processing in a background task and emits answer deltas before `external_channel.completed`. Provider deltas are buffered per model attempt until the final text passes display-quality rejection checks, so generic orchestration acknowledgements are not streamed to third-party clients. Model-pool limit/error/timeout/rejection paths emit `external_channel.answer_retrying` with public `phase=answering`, `status=retrying`, sanitized display text, and `status_url`. Static-page/report follow-up logic and JSON `/events` remain on the existing path. Verified with `cargo test -p platform-api generic_chat_page_event_stream_can_emit_live_answer_delta_without_final_duplication --lib`, `cargo test -p platform-api generic_chat_page_event_stream_retries_without_leaking_rejected_live_delta --lib`, the existing stream endpoint test, `cargo check -p platform-api`, and `cargo test -p llm-gateway --lib`.

**2026-06-06 status:** Main-site continue live SSE wiring completed locally behind the existing default-off `ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED=true` flag. `/v1/assistant-runs/{run_id}/continue/stream` now validates and loads the visible run, spawns the continue worker, passes `AssistantRunLiveDeltaSink` into the normal assistant-chat provider path, streams `assistant_run.delta`, and emits `assistant_run.completed` without repeating the final full text when live deltas were seen. JSON continue and background model-completion recovery remain non-streaming. Verified with `cargo fmt --check -p platform-api`, `cargo test -p platform-api assistant_run_sse --lib`, `cargo test -p platform-api assistant_run_continue_sse --lib`, `cargo test -p platform-api assistant_run_live --lib`, `cargo test -p platform-api assistant_run_continue --lib`, `cargo test -p platform-api external_channel_stream_resume --lib`, `cargo test -p platform-api external_channel_public_stream --lib`, `cargo test -p platform-api generic_chat_page_event_stream_can_emit_live_answer_delta_without_final_duplication --lib`, and `cargo check -p platform-api`.

### Task 5: Unify Status Mapping Across Three Exits

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Rules:**

The same run state must produce the same public status in:

- `POST /events`
- `POST /events/stream`
- `GET /assistant-runs/{assistant_run_id}/reply`

**Steps:**

1. Keep using `external_channel_public_response` and `external_channel_public_reply`.
2. Add a single status mapper:

```rust
fn external_channel_public_task_status(raw_status: &str, card_status: Option<&str>) -> PublicTaskState
```

3. Make all three exits call this mapper.
4. Ensure slow/retrying/queued/background-running states remain `processing`.
5. Tests:
   - static page queued returns same public card in JSON and SSE.
   - published artifact returns same `artifact_links[0]`.
   - cancelled is the only common path that can become final failure.

**Commands:**

```powershell
cargo test -p platform-api external_channel_public_response --lib
cargo test -p platform-api external_channel_static_page --lib
```

**2026-06-02 status:** Completed locally. Static-page public replies now promote any valid `artifact_links[0]`, `card.public_url`, `card.generated_artifact_url`, or `card.artifact_public_url` to public `static_page_published` across `/events`, `/events/stream`, and `/assistant-runs/{assistant_run_id}/reply`. Internal executor/image details remain sanitized by the existing public-card filter. Verified with `cargo test -p platform-api external_channel_public_response_promotes_static_page_artifact_url_to_published --lib`.

### Task 6: Immediate Artifact Link Streaming

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Steps:**

1. In the follow loop for static pages, detect first appearance of:
   - `reply.artifact_links[0]`
   - `reply.card.public_url`
   - `reply.card.generated_artifact_url`

2. Emit `external_channel.static_page_published` immediately.
3. End stream with `completed` and `done` after the public URL is emitted, unless there is an explicit reason to keep the stream alive.
4. Leave background optimization to polling/internal events.
5. Tests:
   - published URL emitted as soon as event exists.
   - stream does not wait for unrelated internal events.

**Commands:**

```powershell
cargo test -p platform-api external_channel_static_page_sse_progress --lib
```

**2026-06-02 status:** Completed locally. The static-page SSE follow loop now treats a valid generated-artifact URL as a terminal publish signal, emits `external_channel.static_page_published`, and does not wait for unrelated background optimization before ending the stream with `done`. Verified with `cargo test -p platform-api external_channel_static_page_sse_treats_artifact_url_as_terminal_publish --lib`.

## P1 Plan

### Task 7: Resumable Needs-Input State

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Behavior:**

When DataMax lacks required information, it should return:

```json
{
  "reply_type": "requires_confirmation",
  "task_status": "needs_input",
  "text": "还需要确认总部/店总对应的门店范围。",
  "card": {
    "status": "needs_input",
    "missing_fields": ["user_role_scope_mapping"],
    "resume_action": "provide_missing_info",
    "status_url": "..."
  }
}
```

**Steps:**

1. Add `needs_input` as a public status distinct from failure.
2. Store missing-field requirements in assistant run events.
3. On the next message in the same conversation, load the pending requirement and attach the user's answer to the original run context.
4. Tests:
   - needs-input state persists.
   - follow-up message resumes instead of starting unrelated flow.
   - model receives the missing information as scoped context.

**2026-06-02 status:** Completed locally. External replies now convert `evidence_state.recovery_followup.status=needs_user_clarification` into a public `needs_input` card with `question`, `missing_fields`, `resume_action`, and `can_continue_same_conversation`; answer-first recovery follow-ups remain normal answers. `/events/stream` emits `external_channel.needs_input` before `completed` so third-party UIs can show a structured prompt instead of treating the turn as a silent failure. Verified with `cargo test -p platform-api external_channel_recovery_followup_becomes_needs_input_reply_and_sse_event --lib`.

### Task 8: Main-Site Streaming UI Alignment

**Files:**
- Modify: `apps/web/app/**`
- Modify: any chat API proxy under `apps/web/app/api/v3/**`
- Test manually with browser and API smoke

**Behavior:**

- Main-site chat should show the same phases as third-party streaming.
- User-facing text remains generic and does not expose internal generation details.
- Main-site can show richer debug details only inside protected observability panels.

**Steps:**

1. Find current main-site chat stream consumer.
2. Add renderer for envelope v1.
3. Render `display_text` for progress.
4. Render `answer_delta` as the assistant bubble.
5. Render artifact link immediately when present.
6. Verify no duplicate assistant bubble is created when `completed` arrives.

**2026-06-02 status:** Completed locally. Main-site `/assistant-runs/stream` and `/assistant-runs/{id}/continue/stream` now emit compatible `schema=v3.assistant_run.sse.v1` envelopes on `assistant_run.accepted` and `assistant_run.completed` while preserving legacy top-level `response`. The main chat UI consumes `assistant_run.*` phase `display_text` as a temporary assistant bubble until answer deltas arrive, and appends generated-artifact links from stream events without duplicating the final answer bubble. Verified with `cargo test -p platform-api assistant_run_sse --lib`, `cargo check -p platform-api`, `npm --prefix apps/web run build`, and `git diff --check`.

### Task 9: Observability Timeline

**Files:**
- Modify: `apps/web/app/external-integrations/**` or existing centralized observability page
- Modify: platform API if a read endpoint is missing

**Behavior:**

- Observability page lists external conversations.
- Selecting a conversation loads stream timeline lazily.
- No permanent resource consumption when not opened.

**Steps:**

1. Add endpoint or reuse existing run detail endpoint for public stream events.
2. UI loads timeline only after a run/conversation is selected.
3. Show event sequence, phase, status, display text, and artifact links.
4. Hide internal raw payload by default; protected debug mode can show sanitized JSON.

**2026-06-02 status:** Completed locally. Added protected `GET /v1/external/conversation-tests/{event_id}/timeline`, returning a sanitized public timeline with sequence, phase, status, display text, artifact links, and optional `debug=1` redacted payloads. The centralized external observability page keeps conversation tests collapsed by default, loads only the selected conversation timeline, and fetches debug JSON only after the operator asks for it. Verified with `cargo test -p platform-api external_conversation_timeline --lib`, `cargo check -p platform-api`, `node --test app/lib/external-integrations.test.mjs app/lib/platform-api.test.mjs`, `npm --prefix apps/web run build`, and `git diff --check`.

## P2 Plan

### Task 10: Third-Party SDK Examples

**Files:**
- Modify: `docs/integrations/third-party-integration-api.zh-CN.md`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Regenerate: published HTML files

**Examples:**

1. Browser `EventSource` style if using GET-compatible proxy.
2. `fetch` streaming POST reader.
3. Server-side polling fallback using `status_url`.
4. Reconnect with `Last-Event-ID`.

**2026-06-02 status:** Completed locally. Added copyable streaming examples to the pure and complete third-party docs: direct `fetch` POST SSE reader, reconnect with the same `idempotency_key` plus `stream_since_sequence`, `status_url` polling fallback, and browser `EventSource` through a third-party GET proxy. Regenerated published HTML/Markdown copies and confirmed external docs do not expose internal generation-provider or executor details. Verified with `npm run build:pure-third-party-guide-html`, `npm run check:pure-third-party-guide-html`, `npm run test:pure-third-party-guide-html`, sensitive-term `rg`, and `git diff --check`.

### Task 11: 10-User Streaming Smoke

**Files:**
- Create: `scripts/smoke/external-channel-streaming-10way.mjs`
- Add npm script in `package.json`

**Checks:**

- 10 simultaneous normal Q&A streams.
- 3 simultaneous static-page/report streams.
- 2 simulated disconnect/reconnect flows.
- No duplicate final assistant messages.
- All long tasks either publish URL or return `continue_polling`.

**2026-06-02 status:** Completed locally. Added `scripts/smoke/external-channel-streaming-10way.mjs` and npm script `smoke:external-channel-streaming-10way`. The smoke runs 10 normal `/events/stream` Q&A tasks, 3 static-page/report streams, and 2 disconnect/reconnect flows by default; it validates single final completion, no duplicate final assistant message, reconnect replay after the last sequence, and static-page long tasks ending with an artifact URL or `continue_polling`. It writes a JSON report under `target/external-channel-streaming-10way-smoke`. Verified with `node --check`, `npm run smoke:external-channel-streaming-10way -- --help`, a zero-request dry run, and `git diff --check`.

### Task 12: Failure And Timeout Policy Audit

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify docs as needed

**Rules:**

- Model timeout: return processing or needs-input if resumable; do not emit fake system fallback.
- Retrieval insufficient: expand supply; if still insufficient, answer with limitation or ask a resumable follow-up.
- Static page timeout: emit `continue_polling`.
- Publish retry: emit `processing` with retry status.
- Cancelled: emit final cancelled/failed only when no background continuation exists.

**2026-06-02 status:** Completed locally for the static-page/report public status layer. `static_page_publish_cancelled` now stays resumable when the response card shows background continuation signals such as retryable runtime state or `poll_after_seconds`; SSE emits `external_channel.static_page_continue_polling` instead of a terminal issue, and JSON top-level `reply.task_status` remains `processing`. Cancellation without continuation still returns final `failed`. Updated pure/full third-party docs to clarify that only top-level `reply.task_status=failed` is non-continuable. Verified with `cargo test -p platform-api external_channel_static_page --lib`.

## Acceptance Criteria

- External docs and public HTML do not contain internal generation-provider or executor details.
- `/events/stream` emits envelope v1 with stable sequence numbers.
- `Last-Event-ID` or `since_sequence` can replay missed events.
- Normal Q&A can stream token deltas when model gateway supports it.
- Non-streaming model providers still emit useful phase progress and a final answer.
- `/events`, `/events/stream`, and `/reply` return consistent public status and artifact links.
- Slow static page/report generation never returns false failure solely due to SSE timeout.
- Once a `public_url` exists, it appears in stream before the client needs to poll again.
- Needs-input states can be resumed by the user's next message.
- Main-site chat can consume the same public event contract.
- Observability timeline loads only when selected.

## Verification Matrix

| Area | Command / Check | Expected |
| --- | --- | --- |
| Platform API compile | `cargo check -p platform-api` | Pass; existing warnings acceptable only if unrelated |
| Static page stream | `cargo test -p platform-api external_channel_static_page --lib` | Pass |
| Stream envelope | `cargo test -p platform-api external_channel_sse --lib` | Pass |
| Resume | `cargo test -p platform-api external_channel_stream_resume --lib` | Pass |
| Q&A stream | `cargo test -p platform-api generic_chat_page_event_stream_can_emit_live_answer_delta_without_final_duplication --lib` | Pass |
| Q&A retry stream | `cargo test -p platform-api generic_chat_page_event_stream_retries_without_leaking_rejected_live_delta --lib` | Pass |
| Docs render | `npm run build:pure-third-party-guide-html && npm run check:pure-third-party-guide-html` | Public docs up to date |
| Public docs secrecy | `rg -n "Image2|GPT-Image|Codex|Cloudflare|效果图|生图" docs/integrations apps/web/public/external-integrations` | No matches in public docs |
| 8 server smoke | request `/events/stream` with a test message | `started`, progress/delta, `completed`, `done` |
| Reconnect smoke | disconnect after sequence N, reconnect with `Last-Event-ID` | only events after N replay |

## Deployment Notes

- Build release API on 8 server with:

```bash
cd /srv/aiv3/repo
CC=clang CXX=clang++ cargo build --release -p platform-api
pnpm --filter @ai-data-platform-v3/web build
systemctl restart aiv3-platform-api.service aiv3-web.service
systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-static-page-worker.service aiv3-codex-host-agent.service
```

- Smoke public docs after deploy:

```bash
curl -fsSL https://v3.elepcloud.com/external-integrations/third-party-integration-api.zh-CN.html >/tmp/full.html
curl -fsSL https://v3.elepcloud.com/external-integrations/pure-third-party-integration-guide.zh-CN.html >/tmp/simple.html
```

## Suggested Commit Slices

1. `Add external stream event envelope`
2. `Persist external stream timeline`
3. `Support external stream resume`
4. `Stream external chat answer deltas`
5. `Unify external stream status mapping`
6. `Emit artifact links immediately in streams`
7. `Document streaming session contract`

## Open Questions

- 是否要把 envelope v1 放进 `contracts` 公开契约，还是先作为 `platform-api` 局部 JSON 约定？
- 主站调试态是否允许显示更细内部事件，还是和第三方完全一致，只在受保护观测页显示内部细节？
- 第三方是否能设置 `Last-Event-ID` header；如果不能，优先支持 `since_sequence` query/body。
- 8 服务器默认 GPT-5.5 通道是否开启 `EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED=true` 灰度，还是先用观测环境跑 10 路 smoke 后再打开？
- live answer delta 是否需要持久化用于断线重放；当前只持久化公共进度事件，最终答案仍可通过 `/reply` 获取。
