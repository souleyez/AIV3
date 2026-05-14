# External Action Result Callback Smoke - 2026-05-14

## Target

- Deployment target: `8服务器`
- Repository path: `/srv/aiv3/repo`
- Validated commit: `7b8dc4a`
- Runner: `scripts/run-external-third-party-gateway-smoke.sh`
- Database environment: platform service environment from `/etc/aiv3/aiv3.env`

## Result

Status: passed.

The deployment target ran:

```bash
bash scripts/run-external-third-party-gateway-smoke.sh
```

The smoke started the standalone Node third-party mock gateway, validated signed outbound dispatch, and then ran the database-backed action result callback contract through the platform API router.

## Passed Checks

```text
cargo test -p platform-api external_action_dispatch_posts_to_external_mock_gateway_from_env --lib -- --nocapture
cargo test -p platform-api external_action_result_callback_records_redacted_summary --lib -- --nocapture
```

Result: both checks passed with `1 passed; 0 failed`.

Gateway verification summary:

```json
{
  "action_id": "act-external-gateway-001",
  "action_type": "external_business_action.invoke",
  "bearer_valid": true,
  "signature_valid": true,
  "body_hash_valid": true
}
```

This confirms:

- V3 can dispatch a confirmed external action to an independently running third-party gateway.
- The gateway receives a request with valid dispatch-specific Bearer authentication, HMAC signature, and body hash.
- Third-party action result callbacks are accepted through `POST /v1/external/channels/{connection_id}/actions/{action_id}/result`.
- V3 verifies channel ownership and the dispatched `external_request_id`.
- V3 records callback status, idempotency key, safe code metadata, and structural result summaries without storing arbitrary third-party message text or raw result values.

## Local Checks

Before deployment-target validation, the local workspace ran:

```text
cargo test -p contracts external_action --lib
cargo test -p platform-api "external_action_result" --lib
git diff --check
```

The local Rust checks compiled and passed. The deployment target provided the PostgreSQL-backed verification path for the callback route.
