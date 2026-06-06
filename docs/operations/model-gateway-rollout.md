# Model Gateway Rollout Runbook

**Scope:** DataMax main-system model pool for ordinary assistant and third-party channel replies.

This runbook keeps observation lightweight. The goal is to make the model pool usable in production without changing document parsing, indexed state, datasets, reports, or HTML artifact flows.

## Operator Access

The model pool routes require a logged-in DataMax user plus one of these server-side allowances:

```text
MODEL_GATEWAY_OPERATOR_EMAILS=ops@example.com,owner@example.com
MODEL_GATEWAY_OPERATOR_ROLES=admin,operator,model_gateway_operator
MODEL_GATEWAY_OPERATOR_ALLOW_ANY_SIGNED_IN=false
```

Use `MODEL_GATEWAY_OPERATOR_ALLOW_ANY_SIGNED_IN=true` only for local development. Production should use explicit emails or roles.

## Preflight

1. Deploy with the model pool page available, but keep runtime routing in `observe_only` or disabled.
2. Confirm `GET /v1/model-gateway/status` works for an operator and returns no API keys, raw auth env names, bearer tokens, or credentialed URLs.
3. Create the provider profiles from the main-system "模型池" page.
4. Run each profile test from the page. The test is a real short provider probe. A profile is not ready for `active` until it returns `ok`.
5. Keep per-profile concurrency and rate budgets conservative for the first rollout.

## Runtime Modes

```text
LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=observe_only
LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=shadow_eval
LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=canary
LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=active
```

`observe_only` and `shadow_eval` must not change the real user-visible reply. Use them to confirm profile health, latency, formatting pass/fail, and retry reasons.

Only use `active` after choosing a narrow scope:

```text
LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE_CONNECTIONS=generic-chat-main
LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE_TENANTS=tenant-ext-001
LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE_PLATFORMS=generic_chat
LLM_GATEWAY_LANE_ASSISTANT_CHAT_CANARY_PERCENT=5
```

For initial production use, prefer one connection or one tenant before platform-wide activation.

## 20-Way External Chat Profile

For the 8-server third-party ordinary-chat lane, use Right Code as the default low-latency provider and keep MiniMax as the fallback profile. This is an internal routing/profile setup only; it does not change third-party URLs, auth, or request/response fields.

Recommended database profiles for lane `assistant_chat`:

| Profile | Provider | Model | Priority | Max concurrency | RPM | Timeout |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| `rightcode-gpt-5-5-default` | `rightcode` | `gpt-5.5` | 100 | 20 | 120 | 20000 ms |
| `minimax-m2-7-fallback` | `minimax` | `MiniMax-M2.7` | 50 | 6 | 120 | 120000 ms |

Recommended runtime environment:

```text
LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=active
LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE=true
EXTERNAL_CHANNEL_DIRECT_REPLY_ATTEMPT_TIMEOUT_MS=20000
EXTERNAL_CHANNEL_DIRECT_REPLY_TOTAL_BUDGET_MS=60000
CHAT_SESSION_WORKER_CONCURRENCY=20
STATIC_PAGE_IMAGE2_HTML_CONCURRENCY=5
CODEX_HOST_CLOUDFLARE_CONCURRENCY=2
```

The attempt timeout is intentionally shorter than the total budget. If Right stalls, DataMax should move to the fallback profile while the third-party caller is still waiting, instead of holding the whole request for a long provider timeout.

`CHAT_SESSION_WORKER_CONCURRENCY` controls the main-site chat task worker only. It defaults to `1` for compatibility and can be raised to `20` once the database pool and model profile capacity are sized for the same deployment.

Main-site chat-session turns now resolve the same `assistant_chat` model pool as third-party ordinary chat. Resolution order is enabled database profiles for the task tenant, env pool profiles, then the legacy `CHAT_SESSION_RUNTIME_*` runtime.

Heavy static-page work is isolated behind worker-level caps. `STATIC_PAGE_IMAGE2_HTML_CONCURRENCY` defaults to `1` and should be raised to `5` for normal Image2/HTML capacity. `CODEX_HOST_CLOUDFLARE_CONCURRENCY` defaults to `1` and should be raised only to `2` for Cloudflare Codex fallback.

Recommended 8-server database pool caps for the 20-way profile:

```text
PLATFORM_DATABASE_MAX_CONNECTIONS=5
PLATFORM_API_DATABASE_MAX_CONNECTIONS=20
CHAT_SESSION_DATABASE_MAX_CONNECTIONS=10
STATIC_PAGE_DATABASE_MAX_CONNECTIONS=8
CODEX_HOST_DATABASE_MAX_CONNECTIONS=4
ASSISTANT_RUN_DATABASE_MAX_CONNECTIONS=5
RETRIEVAL_WORKER_DATABASE_MAX_CONNECTIONS=5
INGEST_WORKER_DATABASE_MAX_CONNECTIONS=5
```

The service-specific value wins over `PLATFORM_DATABASE_MAX_CONNECTIONS`. Invalid or missing values fall back to `10`; values above `100` are clamped.

## Profile Precedence

Enabled database profiles for a lane are preferred over env-only pool profiles. Env config remains the bootstrap and fallback path when no enabled DB profile exists.

Before switching to `active`, verify the profiles shown on the model pool page are the exact profiles intended for the lane. Disable stale DB profiles instead of relying on env values to override them.

## Smoke

First verify the operator-only model gateway surface itself. Use a real logged-in
operator cookie, or the existing main-system local-key login path. Do not pass
third-party bearer tokens to this smoke; they are a different contract.

```bash
npm run smoke:model-gateway-operator -- \
  --base-url http://127.0.0.1:3000 \
  --cookie "aidp_v3_session=..."
```

Alternative using the existing local-key login route:

```bash
MODEL_GATEWAY_OPERATOR_SMOKE_EMAIL=ops@example.com \
MODEL_GATEWAY_OPERATOR_SMOKE_LOCAL_KEY="<operator local key>" \
npm run smoke:model-gateway-operator -- \
  --base-url http://127.0.0.1:3000
```

Add `--run-profile-test` only when the operator intends to consume a real short
provider probe. Without credentials, `--allow-missing-credentials` records only
the unauthenticated `401 auth_session_required` guard and leaves authenticated
checks pending; it is not a production pass.

Current 8-server Task 5 status on 2026-06-06:

- repository head checked: `8f84176dc731`;
- unauthenticated guard rerun against `https://v3.elepcloud.com` passed with `401 auth_session_required`;
- receipt: `/srv/aiv3/repo/target/model-gateway-operator-smoke-no-credentials-8f84176/20260606124152.md`;
- checked server env files did not have an operator smoke cookie, operator smoke email, operator smoke local key, or main assistant streaming smoke cookie configured;
- authenticated profile/status validation remains pending until a legitimate operator session or email plus local-key login is supplied.

After profile tests pass, run local contract smoke tests:

```bash
scripts/run-external-direct-reply-smoke.sh
```

Then run a real external-channel smoke against the target service:

```bash
MODEL_GATEWAY_SMOKE_API_BASE_URL=http://127.0.0.1:8080 \
MODEL_GATEWAY_SMOKE_CONNECTION_ID=generic-chat-main \
MODEL_GATEWAY_SMOKE_BEARER=<optional-token> \
MODEL_GATEWAY_SMOKE_CONCURRENCY=10 \
scripts/run-model-gateway-rollout-smoke.sh
```

Repeat with `MODEL_GATEWAY_SMOKE_CONCURRENCY=20` before expanding beyond the first tenant or connection. The script writes a small JSON and Markdown report under `target/model-gateway-rollout-smoke`.

For a third-party SSE-style smoke that mirrors the current public external-channel contract, run:

```bash
node scripts/smoke/external-channel-20way.mjs \
  --base-url http://127.0.0.1:3000 \
  --connection-id generic-chat-main \
  --bearer "$EXTERNAL_CHANNEL_BEARER" \
  --concurrency 20
```

The script creates unique `conversation_external_id` and `message_external_id` values, sends ordinary text events to `/v1/external/channels/{connection_id}/events/stream`, and reports completion, accepted/answered status, latency, and error counts.

## Rollback

Fast rollback does not require a code deploy:

1. Set `LLM_GATEWAY_LANE_ASSISTANT_CHAT_MODE=observe_only` or clear the active connection/tenant/platform env filters.
2. Restart the platform API process so env changes take effect.
3. Disable the bad profile from the model pool page.
4. If DB profiles are the problem, disable all enabled profiles for the lane so the old env routing path can take over.
5. Keep the model pool page available for diagnosis, but do not leave the lane in `active`.

## Known Limits

- Budgets are process-local. They are good enough for first rollout, but they do not coordinate quota across multiple platform-api replicas.
- Profile tests are short probes, not quality evaluation. They prove auth, endpoint, model id, timeout, and a non-empty reply.
- Provider-specific global env paths can still have provider-owned timeout behavior. OpenAI-compatible profiles with `base_url` use the profile timeout most predictably.
