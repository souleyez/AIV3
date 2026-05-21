# Model Gateway Rollout Runbook

**Scope:** V3 main-system model pool for ordinary assistant and third-party channel replies.

This runbook keeps observation lightweight. The goal is to make the model pool usable in production without changing document parsing, indexed state, datasets, reports, or HTML artifact flows.

## Operator Access

The model pool routes require a logged-in V3 user plus one of these server-side allowances:

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

## Profile Precedence

Enabled database profiles for a lane are preferred over env-only pool profiles. Env config remains the bootstrap and fallback path when no enabled DB profile exists.

Before switching to `active`, verify the profiles shown on the model pool page are the exact profiles intended for the lane. Disable stale DB profiles instead of relying on env values to override them.

## Smoke

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
