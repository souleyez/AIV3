# Model Gateway Concurrency Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enhance the DataMax model gateway so operators can add model API profiles in the UI, run different models concurrently across different requests, and see each model's usage and health without implementing P3 racing/hedged requests.

**Architecture:** Keep DataMax request handling synchronous for normal chat, but put a model gateway scheduler in front of provider calls. Model provider profiles are managed from a dedicated "模型池" page in the main DataMax system, placed after "审计" in the top workspace navigation, and stored in PostgreSQL; env config stays as a bootstrap/fallback path. The scheduler selects one available profile per request, enforces lane/profile concurrency, queues briefly when saturated, falls back sequentially on retryable failures, and exposes per-profile usage/health.

**Tech Stack:** Rust, Axum, Tokio, `crates/llm-gateway`, `crates/platform-api`, Next.js main system model pool page, PostgreSQL assistant run events.

---

## Scope

Included:
- P0: UI-managed model API profiles with recommended presets per provider/model.
- P0: cross-model API pool for normal routing.
- P1: lane/profile limits, queueing, timeout, circuit breaker.
- P2: sequential fallback and per-model usage/health observability.
- P2: non-disruptive quality optimization through shadow evaluation, canary routing, and per-lane enablement.

Excluded:
- P3 racing/hedged requests where one user request is sent to multiple models at the same time.
- New paid upstream quota, new server purchase, or external queue service.

## Profile Configuration Model

Primary path:
- The main DataMax system provides a dedicated "模型池" page after "审计" in the workspace navigation.
- Operators can add, edit, disable, and test model profiles.
- Profiles are stored in PostgreSQL and loaded by platform-api at runtime.
- Env config remains available for bootstrap and emergency rollback.
- The external observability page may show a small read-only summary later, but it must not be the management surface.

Profile fields:

| Field | Meaning |
| --- | --- |
| `profile_id` | Stable internal profile id, for example `openclaw-main` or `minimax-fast`. |
| `display_name` | Human-readable name shown in the UI. |
| `lane` | Usage lane, initially `assistant_chat`; later can add parse, report, or vision lanes. |
| `provider_id` | Provider key, for example `openclaw`, `minimax`, `openai-compatible`, or `custom`. |
| `model_id` | Upstream model name. |
| `base_url` / `api_path` / `wire_api` | OpenAI-compatible or provider-specific endpoint settings. |
| `auth_mode` | `env_key` for MVP; optional write-only secret binding later. |
| `auth_env_key_name` | Environment variable name that contains the API key. Never return the resolved secret to the browser. |
| `recommended_preset` | Preset key used to prefill timeout, concurrency, token limits, and capabilities. |
| `max_concurrency` | Maximum simultaneous calls to this profile. |
| `rpm_limit` / `tpm_limit` | Optional request/token budgets per minute. |
| `timeout_ms` | Single upstream request timeout. |
| `priority` | Higher priority is tried first when multiple profiles are available. |
| `enabled` | Disabled profiles are visible but not selected for traffic. |
| `capabilities` | Chat, streaming, JSON mode, long context, vision, or other flags. |

Recommended preset behavior:
- Select provider/model first, then the UI prefills recommended concurrency, timeout, RPM/TPM, capabilities, and wire API.
- Operators can override any numeric limit before saving.
- Start with presets for current OpenClaw/default, MiniMax-M2.7, OpenAI-compatible chat, and custom OpenAI-compatible endpoint.
- Store the selected preset key with the profile so future UI can show "recommended" versus "overridden" fields.

Per-model usage and health display:
- current active calls;
- current queue depth;
- configured max concurrency;
- request count in current minute/hour/day;
- estimated token count when available;
- success, retryable failure, timeout, and 429 counts;
- p50/p95 latency;
- circuit breaker state, opened-until time, and consecutive failure count;
- last success time and last failure reason;
- profile enabled/disabled state.

Important concurrency rule:
- Different requests can run through different models at the same time.
- One request still uses one selected profile at a time.
- Fallback is sequential: failed/429/timeout profile -> next available profile.
- P3 racing/hedged requests remain out of scope.

## Non-Disruptive Quality Optimization Rules

Quality optimization must not affect unrelated DataMax processes by default.

Rules:
- Additive migrations only; do not rewrite existing assistant, dataset, parse, or artifact tables unless required.
- Existing chat, document parse, dataset indexing, report generation, and third-party APIs keep their current route unless a model pool lane is explicitly enabled.
- New model pool routing starts in `disabled` or `observe_only` mode.
- Shadow evaluation can score prompts, model profiles, latency, and response shape without returning shadow output to users.
- Canary rollout must be scoped by lane, tenant, channel, or explicit profile id.
- Rollback must be possible by disabling DB profiles or clearing env pool config.
- No global provider/model env variable should be replaced until the pool has been stable in production.

Quality improvements that are safe to build in parallel:
- model-specific prompt templates;
- output format validation and repair;
- response quality scoring stored as events;
- retrieval evidence compression;
- timeout/retry tuning per model profile;
- per-profile temperature/max token recommendations;
- model selection rules for normal chat versus HTML/report generation.

Do not change these paths during the first model pool rollout:
- document parse lifecycle behavior;
- existing indexed document state;
- dataset membership and visibility behavior;
- published external integration documents;
- static page artifact rendering contracts.

## Bootstrap Env Fallback Contract

```env
LLM_GATEWAY_LANE_ASSISTANT_CHAT_PROFILES=OPENCLAW_MAIN,MINIMAX_FAST
LLM_GATEWAY_LANE_ASSISTANT_CHAT_MAX_CONCURRENCY=30
LLM_GATEWAY_LANE_ASSISTANT_CHAT_QUEUE_LIMIT=200
LLM_GATEWAY_LANE_ASSISTANT_CHAT_QUEUE_TIMEOUT_MS=3000

LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_PROVIDER_ID=openclaw
LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_MODEL_ID=default
LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_PRIORITY=100
LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_RATE_LIMIT_CONCURRENCY=20
LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_TIMEOUT_MS=30000

LLM_GATEWAY_PROFILE_MINIMAX_FAST_PROVIDER_ID=minimax
LLM_GATEWAY_PROFILE_MINIMAX_FAST_MODEL_ID=MiniMax-M2.7
LLM_GATEWAY_PROFILE_MINIMAX_FAST_PRIORITY=80
LLM_GATEWAY_PROFILE_MINIMAX_FAST_RATE_LIMIT_CONCURRENCY=10
LLM_GATEWAY_PROFILE_MINIMAX_FAST_TIMEOUT_MS=30000

LLM_GATEWAY_CIRCUIT_BREAKER_FAILURES=5
LLM_GATEWAY_CIRCUIT_BREAKER_COOLDOWN_MS=60000
```

Rules:
- If PostgreSQL has enabled model profiles for a lane, prefer those profiles.
- If no enabled DB profiles exist, use `LLM_GATEWAY_LANE_*_PROFILES`.
- If no `LLM_GATEWAY_LANE_*_PROFILES` is configured, keep current `LLM_GATEWAY_ROUTE_*` and `*_RUNTIME_*` behavior.
- Each request selects one profile only.
- Fallback is sequential: failed/429/timeout profile -> next available profile.
- Queueing happens before provider call, not while holding a database connection.

---

### Task 1: Add Model Gateway Profile Storage

**Files:**
- Create: `crates/storage/migrations/0011_model_gateway_profiles.sql`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: `crates/storage/src/lib.rs`

**Step 1: Write failing migration registry test**

Add a test that asserts migration `0011_model_gateway_profiles.sql` is registered after `0010_dataset_document_memberships.sql`.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p storage model_gateway_profiles_migration_is_registered
```

Expected: fail because the migration does not exist yet.

**Step 3: Add migration**

Create tables:
- `model_gateway_profiles`
- `model_gateway_profile_events`

Minimum `model_gateway_profiles` columns:
- `id uuid primary key`;
- `tenant_id uuid not null`;
- `profile_id text not null`;
- `display_name text not null`;
- `lane text not null`;
- `provider_id text not null`;
- `model_id text not null`;
- `base_url text`;
- `api_path text`;
- `wire_api text not null default 'openai-compatible'`;
- `auth_mode text not null default 'env_key'`;
- `auth_env_key_name text`;
- `recommended_preset text`;
- `max_concurrency integer`;
- `rpm_limit integer`;
- `tpm_limit integer`;
- `timeout_ms integer`;
- `priority integer not null default 100`;
- `enabled boolean not null default true`;
- `capabilities jsonb not null default '{}'::jsonb`;
- `created_at timestamptz not null default now()`;
- `updated_at timestamptz not null default now()`;
- unique key on `(tenant_id, profile_id)`.

Minimum `model_gateway_profile_events` columns:
- `id uuid primary key`;
- `tenant_id uuid not null`;
- `profile_id text not null`;
- `lane text not null`;
- `event_type text not null`;
- `latency_ms integer`;
- `input_tokens integer`;
- `output_tokens integer`;
- `error_kind text`;
- `created_at timestamptz not null default now()`.

Do not store raw API keys in these tables for MVP.

**Step 4: Run tests**

Run:

```bash
cargo test -p storage
cargo test -p contracts
```

**Step 5: Commit**

```bash
git add crates/storage/migrations/0011_model_gateway_profiles.sql crates/storage/src/lib.rs crates/contracts/src/lib.rs
git commit -m "Add model gateway profile storage"
```

---

### Task 2: Add Profile Management API and Presets

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/storage/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Add tests for:
- listing recommended presets;
- creating a profile from a preset;
- updating profile limits;
- disabling a profile;
- testing a profile without returning secrets.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p platform-api model_gateway_profile
```

Expected: fail because the API and contracts do not exist.

**Step 3: Add contracts**

Add request/response views:
- `ModelGatewayPresetView`
- `ModelGatewayProfileView`
- `ModelGatewayProfileCreateRequest`
- `ModelGatewayProfileUpdateRequest`
- `ModelGatewayProfileTestRequest`
- `ModelGatewayProfileTestResponse`

`ModelGatewayProfileView` must include:
- config fields safe for UI display;
- derived `has_secret` or `auth_env_key_name`;
- no raw API key;
- no bearer token;
- no request body containing secrets.

**Step 4: Add protected routes**

Add routes behind the main system operator/admin access gate, not the external observability one-time access gate:

```text
GET    /v1/model-gateway/presets
GET    /v1/model-gateway/profiles
POST   /v1/model-gateway/profiles
PATCH  /v1/model-gateway/profiles/{profile_id}
POST   /v1/model-gateway/profiles/{profile_id}/disable
POST   /v1/model-gateway/profiles/{profile_id}/test
```

Preset MVP:
- `openclaw/default`: conservative concurrency, 30s timeout;
- `minimax/MiniMax-M2.7`: lower default concurrency, 30s timeout;
- `openai-compatible/chat`: generic OpenAI-compatible endpoint;
- `custom`: no assumptions except safe defaults.

**Step 5: Add storage helpers**

Add typed storage methods for:
- list enabled profiles by lane;
- list all profiles for UI;
- create/update/disable profile;
- record profile event;
- summarize recent profile usage.

**Step 6: Run tests**

Run:

```bash
cargo test -p storage model_gateway_profile
cargo test -p platform-api model_gateway_profile
cargo check -p platform-api
```

**Step 7: Commit**

```bash
git add crates/contracts/src/lib.rs crates/storage/src/lib.rs crates/platform-api/src/lib.rs
git commit -m "Add model gateway profile management API"
```

---

### Task 3: Add Main System Model Pool Page

**Files:**
- Create: `apps/web/app/lib/model-gateway.js`
- Create: `apps/web/app/lib/model-gateway.test.mjs`
- Create: `apps/web/app/components/ModelPoolPanel.js`
- Modify: `apps/web/app/components/HomeWorkspaceToolbar.js`
- Modify: `apps/web/app/components/WorkspaceDirectoryPanel.js`
- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/globals.css`

**Step 1: Write failing JS tests**

Add tests that normalize:
- model gateway presets;
- profile list;
- redacted secret fields;
- profile status summary.

**Step 2: Add top navigation entry**

In `apps/web/app/components/HomeWorkspaceToolbar.js`, add the new nav item immediately after `audit`:

```js
{ key: 'model-pool', label: '模型池' }
```

**Step 3: Implement API client helpers**

Add helper calls for:
- `fetchModelGatewayPresets`;
- `fetchModelGatewayProfiles`;
- `createModelGatewayProfile`;
- `updateModelGatewayProfile`;
- `disableModelGatewayProfile`;
- `testModelGatewayProfile`.

These helpers should call the main system proxy paths:
- `/api/v3/model-gateway/presets`;
- `/api/v3/model-gateway/profiles`;
- `/api/v3/model-gateway/status`.

**Step 4: Implement page rendering**

Create `ModelPoolPanel` and render it when `activePage === 'model-pool'`.

The model pool page should include:
- profile table/cards;
- add profile button;
- edit profile drawer/modal;
- provider/model preset selector;
- recommended values preview;
- override inputs for concurrency, RPM, TPM, timeout, priority, enabled;
- write-only secret update or `auth_env_key_name` input;
- test connection button.

In `WorkspaceDirectoryPanel`, add page copy:

```js
'model-pool': {
  title: '模型池',
  subtitle: '统一配置模型 API、并发额度、健康状态和故障切换策略。',
}
```

Then render `ModelPoolPanel` below that page header.

**Step 5: Run tests/build**

Run:

```bash
cd apps/web
node --test app/lib/model-gateway.test.mjs
npm run build
```

**Step 6: Commit**

```bash
git add apps/web/app/lib/model-gateway.js apps/web/app/lib/model-gateway.test.mjs apps/web/app/components/ModelPoolPanel.js apps/web/app/components/HomeWorkspaceToolbar.js apps/web/app/components/WorkspaceDirectoryPanel.js apps/web/app/HomePageClient.js apps/web/app/globals.css
git commit -m "Add main system model pool page"
```

---

### Task 4: Add Gateway Pool Config Parsing

**Files:**
- Modify: `crates/llm-gateway/src/lib.rs`
- Test: `crates/llm-gateway/src/lib.rs`

**Step 1: Write failing tests**

Add tests near existing `ModelRouteRegistry` tests:

```rust
#[test]
fn model_gateway_lane_pool_reads_profile_list_from_env() {
    clear_model_gateway_pool_env("ASSISTANT_CHAT", &["OPENCLAW_MAIN", "MINIMAX_FAST"]);
    std::env::set_var(
        "LLM_GATEWAY_LANE_ASSISTANT_CHAT_PROFILES",
        "OPENCLAW_MAIN,MINIMAX_FAST",
    );
    std::env::set_var("LLM_GATEWAY_LANE_ASSISTANT_CHAT_MAX_CONCURRENCY", "30");
    std::env::set_var("LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_PROVIDER_ID", "openclaw");
    std::env::set_var("LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_MODEL_ID", "default");
    std::env::set_var("LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_RATE_LIMIT_CONCURRENCY", "20");
    std::env::set_var("LLM_GATEWAY_PROFILE_MINIMAX_FAST_PROVIDER_ID", "minimax");
    std::env::set_var("LLM_GATEWAY_PROFILE_MINIMAX_FAST_MODEL_ID", "MiniMax-M2.7");

    let pool = ModelGatewayPoolConfig::from_env(MODEL_LANE_ASSISTANT_CHAT).unwrap();

    assert_eq!(pool.lane, MODEL_LANE_ASSISTANT_CHAT);
    assert_eq!(pool.lane_limits.max_concurrency, Some(30));
    assert_eq!(pool.profiles.len(), 2);
    assert_eq!(pool.profiles[0].provider_id, "openclaw");
    assert_eq!(pool.profiles[0].rate_limit.concurrent_requests, Some(20));
    assert_eq!(pool.profiles[1].provider_id, "minimax");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p llm-gateway model_gateway_lane_pool_reads_profile_list_from_env
```

Expected: fail because `ModelGatewayPoolConfig` does not exist.

**Step 3: Implement minimal config structs**

Add:
- `ModelGatewayLaneLimits`
- `ModelGatewayPoolConfig`
- `model_gateway_lane_env_prefix(lane: &str)`
- `model_gateway_profile_env_prefix(profile_name: &str)`

Use existing helpers:
- `optional_env_u32`
- `optional_env_u64`
- `optional_env_string`
- `split_csv_env`
- `ModelProviderProfile::from_env`

**Step 4: Run tests**

Run:

```bash
cargo test -p llm-gateway model_gateway_lane_pool_reads_profile_list_from_env
cargo test -p llm-gateway
```

**Step 5: Commit**

```bash
git add crates/llm-gateway/src/lib.rs
git commit -m "Add model gateway pool config parsing"
```

---

### Task 5: Add Runtime Limiter State

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Add a unit test for a small pure helper if possible:

```rust
#[tokio::test]
async fn gateway_limiter_rejects_when_lane_queue_is_full() {
    let limiter = GatewayRuntimeLimiter::for_test()
        .with_lane_limit(MODEL_LANE_ASSISTANT_CHAT, 1, 0);
    let first = limiter.acquire_lane(MODEL_LANE_ASSISTANT_CHAT).await.unwrap();
    let second = limiter.acquire_lane(MODEL_LANE_ASSISTANT_CHAT).await;

    assert!(matches!(second, Err(GatewayLimitError::QueueFull)));
    drop(first);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p platform-api gateway_limiter_rejects_when_lane_queue_is_full
```

Expected: fail because limiter does not exist.

**Step 3: Implement limiter**

Add a runtime field to `AppState`:

```rust
gateway_limiter: Arc<GatewayRuntimeLimiter>,
```

Add limiter types in `crates/platform-api/src/lib.rs`:
- `GatewayRuntimeLimiter`
- `GatewayPermit`
- `GatewayLimitError`
- `GatewayProviderStats`

Implementation notes:
- Use `tokio::sync::Semaphore` for lane and provider concurrency.
- Use `tokio::time::timeout` for queue wait.
- Track active/queued counts with atomics or `Mutex<HashMap<...>>`.
- Default lane concurrency should be high enough to preserve current behavior when env is absent.

**Step 4: Run tests**

Run:

```bash
cargo test -p platform-api gateway_limiter_rejects_when_lane_queue_is_full
cargo check -p platform-api
```

**Step 5: Commit**

```bash
git add crates/platform-api/src/lib.rs
git commit -m "Add gateway runtime limiter"
```

---

### Task 6: Route Assistant Chat Through Provider Pool

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/llm-gateway/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing test**

Add a test beside existing external channel fallback tests:

```rust
#[tokio::test]
async fn external_channel_direct_reply_uses_next_profile_after_retryable_failure() {
    // Configure assistant_chat pool with two scripted profiles.
    // First returns retryable provider error or timeout.
    // Second returns "fallback answer".
    // Assert response text is "fallback answer".
    // Assert assistant_run event records both attempted profile ids.
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p platform-api external_channel_direct_reply_uses_next_profile_after_retryable_failure
```

Expected: fail because direct reply only uses current primary/fallback runtime selection.

**Step 3: Implement profile selection**

Update `external_channel_chat_model_or_acceptance_reply` flow:
- Load enabled DB profiles for `MODEL_LANE_ASSISTANT_CHAT`.
- If DB profiles exist, convert them into ordered `ModelGatewayPoolConfig` candidates.
- If no enabled DB profiles exist, load `ModelGatewayPoolConfig::from_env(MODEL_LANE_ASSISTANT_CHAT)`.
- If no DB or env pool profiles are configured, use current behavior unchanged.
- If a pool is configured, build ordered candidate attempts from its profiles.
- For each candidate:
  - acquire lane permit;
  - acquire profile permit;
  - call provider once;
  - on retryable failure, release permit and try next profile;
  - on success, persist runtime manifest with profile id/provider/model.

Do not implement racing.

**Step 4: Preserve existing fallback behavior**

Run existing tests that cover:

```bash
cargo test -p platform-api external_channel_model_reply
```

**Step 5: Commit**

```bash
git add crates/platform-api/src/lib.rs crates/llm-gateway/src/lib.rs
git commit -m "Route external chat through model provider pool"
```

---

### Task 7: Add Circuit Breaker

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn gateway_limiter_skips_provider_during_circuit_breaker_cooldown() {
    let limiter = GatewayRuntimeLimiter::for_test()
        .with_provider_limit("openclaw-main", 10)
        .with_circuit_breaker(2, Duration::from_secs(60));

    limiter.record_provider_failure("openclaw-main");
    limiter.record_provider_failure("openclaw-main");

    assert!(limiter.provider_available("openclaw-main").is_none());
}
```

**Step 2: Implement**

In `GatewayRuntimeLimiter`, track:
- consecutive failures;
- opened_until;
- success resets failure count;
- retryable failures increment failure count;
- non-retryable validation errors should not necessarily open circuit.

Retryable kinds:
- timeout;
- HTTP 429;
- HTTP 5xx;
- upstream unavailable;
- network error.

**Step 3: Run tests**

```bash
cargo test -p platform-api gateway_limiter_skips_provider_during_circuit_breaker_cooldown
cargo test -p platform-api external_channel_direct_reply_uses_next_profile_after_retryable_failure
```

**Step 4: Commit**

```bash
git add crates/platform-api/src/lib.rs
git commit -m "Add model provider circuit breaker"
```

---

### Task 8: Add Gateway Observability API

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn gateway_status_exposes_lane_and_provider_counts_without_secrets() {
    // Build AppState with limiter state.
    // Call GET /v1/model-gateway/status or internal status helper.
    // Assert lane active/queued and provider circuit state.
    // Assert no api key/base token appears in serialized response.
}
```

**Step 2: Implement contracts**

Add views:
- `ModelGatewayStatusView`
- `ModelGatewayLaneStatusView`
- `ModelGatewayProviderStatusView`

Fields:
- lane;
- max_concurrency;
- active_count;
- queued_count;
- queue_limit;
- provider_id/profile_id;
- display_name;
- provider;
- model;
- active_count;
- max_concurrency;
- rpm_limit;
- tpm_limit;
- requests_current_minute;
- requests_current_hour;
- estimated_tokens_current_minute;
- success_count;
- failure_count;
- timeout_count;
- rate_limit_count;
- p50_latency_ms;
- p95_latency_ms;
- circuit_state;
- opened_until;
- last_success_at;
- last_failure_at;
- last_failure_reason;
- enabled.

No raw API key, token, base URL with credentials, or request body.

**Step 3: Implement route**

Add protected route:

```rust
GET /v1/model-gateway/status
```

Use the same main system operator/admin access check as profile management routes.

**Step 4: Run tests**

```bash
cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets
cargo test -p contracts
```

**Step 5: Commit**

```bash
git add crates/contracts/src/lib.rs crates/platform-api/src/lib.rs
git commit -m "Expose model gateway status"
```

---

### Task 9: Wire Live Status Into Model Pool Page

**Files:**
- Modify: `apps/web/app/lib/model-gateway.js`
- Modify: `apps/web/app/components/ModelPoolPanel.js`
- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/globals.css`
- Test: `apps/web/app/lib/model-gateway.test.mjs`

**Step 1: Write failing JS tests**

Add tests for normalizing model gateway status:

```js
test('normalizeModelGatewayStatus hides secrets and keeps lane counts readable', () => {
  const status = normalizeModelGatewayStatus({
    lanes: [{ lane: 'assistant_chat', active_count: 12, queued_count: 3 }],
    providers: [{ profile_id: 'OPENCLAW_MAIN', provider: 'openclaw', model: 'default' }],
  });
  assert.equal(status.lanes[0].activeCount, 12);
  assert.equal(JSON.stringify(status).includes('api_key'), false);
});
```

**Step 2: Implement frontend loading**

In the main system model pool page:
- Fetch `/api/v3/model-gateway/status` while `activePage === 'model-pool'`.
- Refresh status periodically while the page is open.
- Display:
  - lane active/queued;
  - profile active/max;
  - per-profile request/token usage;
  - p50/p95 latency;
  - circuit state;
  - recent failure counts;
  - last success/failure.

**Step 3: Run tests/build**

```bash
cd apps/web
node --test app/lib/model-gateway.test.mjs
npm run build
```

**Step 4: Commit**

```bash
git add apps/web/app/lib/model-gateway.js apps/web/app/components/ModelPoolPanel.js apps/web/app/HomePageClient.js apps/web/app/globals.css apps/web/app/lib/model-gateway.test.mjs
git commit -m "Show live model pool status"
```

---

### Task 10: Deployment Config and Smoke Test

**Files:**
- Modify: `/etc/aiv3/aiv3.env` on 8 server
- No repository commit for secrets

**Step 1: Add conservative bootstrap config**

Start with safe no-budget env fallback limits so the system can boot before UI profiles are created:

```env
LLM_GATEWAY_LANE_ASSISTANT_CHAT_PROFILES=OPENCLAW_MAIN,MINIMAX_FAST
LLM_GATEWAY_LANE_ASSISTANT_CHAT_MAX_CONCURRENCY=20
LLM_GATEWAY_LANE_ASSISTANT_CHAT_QUEUE_LIMIT=100
LLM_GATEWAY_LANE_ASSISTANT_CHAT_QUEUE_TIMEOUT_MS=3000

LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_PROVIDER_ID=openclaw
LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_MODEL_ID=default
LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_PRIORITY=100
LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_RATE_LIMIT_CONCURRENCY=15
LLM_GATEWAY_PROFILE_OPENCLAW_MAIN_TIMEOUT_MS=30000

LLM_GATEWAY_PROFILE_MINIMAX_FAST_PROVIDER_ID=minimax
LLM_GATEWAY_PROFILE_MINIMAX_FAST_MODEL_ID=MiniMax-M2.7
LLM_GATEWAY_PROFILE_MINIMAX_FAST_PRIORITY=80
LLM_GATEWAY_PROFILE_MINIMAX_FAST_RATE_LIMIT_CONCURRENCY=5
LLM_GATEWAY_PROFILE_MINIMAX_FAST_TIMEOUT_MS=30000
```

Then create matching enabled profiles from the main system "模型池" page and confirm the runtime prefers DB profiles.

**Step 2: Build and restart**

```bash
cd /srv/aiv3/repo
git pull --ff-only
npm --prefix apps/web run build
CC=clang CXX=clang++ cargo build --release -p platform-api
systemctl restart aiv3-platform-api.service aiv3-web.service
systemctl is-active aiv3-platform-api.service aiv3-web.service
```

Expected: both services `active`.

**Step 3: Smoke test**

Use a test third-party event against `/v1/external/channels/{connection_id}/events/stream`.

Expected:
- response returns displayable model text;
- main system "模型池" page shows active counts during request;
- provider stats increment after request;
- no secrets appear in UI/API response.

**Step 4: Controlled load smoke**

Run a small local load test first:
- 10 concurrent requests;
- then 20 concurrent requests;
- do not jump directly to 100.

Success criteria:
- no API restart;
- p95 under 30s for normal chat;
- no DB pool timeout;
- no model provider runaway 429 loop;
- queue count returns to zero after the burst.

---

### Task 11: Add Shadow Quality Evaluation Mode

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/components/ModelPoolPanel.js`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Add tests that verify:
- shadow mode records quality/latency events;
- shadow mode does not alter the user-visible assistant response;
- disabling shadow mode stops new quality events;
- quality events never store raw API keys or hidden prompt secrets.

**Step 2: Add profile rollout modes**

Add a per-profile or per-lane rollout mode:
- `disabled`;
- `observe_only`;
- `shadow_eval`;
- `canary`;
- `active`.

`active` is the only mode allowed to affect user-visible model routing.

**Step 3: Record quality events**

Record:
- profile id;
- lane;
- input/output token estimate when available;
- latency;
- output format validity;
- retry/repair count;
- model/provider error kind;
- optional human/operator score.

Store scores as metadata/events, not as a replacement for assistant output.

**Step 4: Show quality signals in Model Pool page**

In "模型池", show:
- rollout mode;
- recent quality score;
- format pass rate;
- repair rate;
- average/p95 latency;
- last shadow evaluation time.

**Step 5: Run tests**

```bash
cargo test -p platform-api model_gateway_shadow_quality
cd apps/web
node --test app/lib/model-gateway.test.mjs
npm run build
```

**Step 6: Commit**

```bash
git add crates/contracts/src/lib.rs crates/storage/src/lib.rs crates/platform-api/src/lib.rs apps/web/app/components/ModelPoolPanel.js apps/web/app/lib/model-gateway.js apps/web/app/lib/model-gateway.test.mjs
git commit -m "Add shadow quality evaluation for model pool"
```

---

## Rollout Recommendation

1. Ship model pool page and profile storage first; no production routing behavior change.
2. Ship config parsing and limiter in `observe_only` mode; record would-throttle metrics but do not reject.
3. Enable `shadow_eval` for quality optimization; do not return shadow output to users.
4. Enable `active` only for one lane or one internal channel first.
5. Start active limits at `assistant_chat=20`, provider split `15/5`.
6. Watch 24 hours of p95, 429, timeout, queue depth, format pass rate, and repair rate.
7. Raise to `30-40` only if upstream model APIs stay clean.

## Final Acceptance Criteria

- DataMax can configure multiple model API profiles for one lane from the main system "模型池" page.
- The "模型池" page appears immediately after "审计" in the main workspace navigation.
- The UI provides recommended presets for common provider/model combinations.
- Operators can add, edit, disable, and test a model profile without exposing raw API keys.
- A single request uses only one model at a time.
- Different requests can concurrently use different enabled model profiles.
- Provider failure falls back sequentially to another configured model API.
- Lane/profile concurrency is enforced.
- Saturation queues briefly or returns a clear overload response.
- Circuit breaker prevents repeated calls to a failing provider.
- The "模型池" page shows current lane/profile status, per-profile usage, and health without secrets.
- Quality optimization can run in observe/shadow mode without changing user-visible outputs.
- Active routing can be enabled by lane/tenant/channel and rolled back without code changes.
- Existing behavior remains unchanged when no DB profile or pool env is configured.
