# External Action Result Callback Smoke - 2026-05-14

## Target

- Deployment target: `8服务器`
- Repository path: `/srv/aiv3/repo`
- Validated commit: `120933a`
- Runner: `scripts/run-external-third-party-gateway-smoke.sh`
- Database environment: platform service environment from `/etc/aiv3/aiv3.env`

## Result

Status: passed.

The deployment target ran:

```bash
bash scripts/run-external-third-party-gateway-smoke.sh
```

The smoke started the standalone Node third-party mock gateway, validated signed outbound dispatch, ran the database-backed action result callback contract through the platform API router, and then asked the standalone gateway to POST an action result back into a temporary V3 HTTP server.

## Passed Checks

```text
cargo test -p platform-api external_action_dispatch_posts_to_external_mock_gateway_from_env --lib -- --nocapture
cargo test -p platform-api external_action_result_callback_records_redacted_summary --lib -- --nocapture
cargo test -p platform-api external_action_gateway_posts_result_callback_to_v3_from_env --lib -- --nocapture
```

Result: all three checks passed with `1 passed; 0 failed`.

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

Gateway callback verification summary:

```json
{
  "callback_status": 200,
  "response_accepted": true,
  "action_id": "act-gateway-callback-001",
  "status": "succeeded"
}
```

This confirms:

- V3 can dispatch a confirmed external action to an independently running third-party gateway.
- The gateway receives a request with valid dispatch-specific Bearer authentication, HMAC signature, and body hash.
- Third-party action result callbacks are accepted through `POST /v1/external/channels/{connection_id}/actions/{action_id}/result`.
- The standalone gateway can actively call the V3 result callback endpoint over HTTP, not only through an in-process router test.
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

## Observability Follow-Up

Follow-up validated commit: `c3f702c`.

The deployment target pulled `c3f702c` and ran:

```text
bash scripts/run-external-third-party-gateway-smoke.sh
node --test app/lib/external-integrations.test.mjs
```

Result: passed.

The gateway smoke still validated signed dispatch plus gateway-to-V3 result callback roundtrip. The web contract test passed 9 checks, including action lifecycle normalization for `waiting_result`, `result_succeeded`, and `result_failed`.

This confirms:

- `GET /v1/external/integrations` can expose observe-only `action_summary` lifecycle counts without breaking gateway smoke.
- The standalone panel can normalize and display result callback states.
- Result failures now surface as operational failures while keeping raw third-party callback content out of UI summaries.

## Audit Filter Follow-Up

Follow-up validated commit: `15d700a`.

The deployment target pulled `15d700a` and ran:

```text
bash scripts/run-external-third-party-gateway-smoke.sh
node --test app/lib/external-integrations.test.mjs
```

Result: passed.

The gateway smoke still validated signed dispatch plus gateway-to-V3 result callback roundtrip. The web contract test passed 10 checks, including fixed audit query encoding for `item_type=action&action_state=result_callback`.

This confirms:

- `GET /v1/external/integrations/{integration_id}/audit` supports fixed `item_type`, `action_state`, and `limit` filters.
- The standalone panel exposes stable filters for all records, action records, result callbacks, waiting-result actions, and failed/blocked actions.
- Invalid or conflicting backend filter combinations are covered by focused Rust tests.

## Action Drilldown Follow-Up

Follow-up validated commit: `a203a54`.

Local validation ran:

```text
cargo test -p platform-api external_integration_audit_filter --lib
node --test app/lib/external-integrations.test.mjs
npm run build
git diff --check
```

The deployment target pulled `a203a54` and ran:

```text
bash scripts/run-external-third-party-gateway-smoke.sh
node --test app/lib/external-integrations.test.mjs
```

Result: passed.

The gateway smoke still validated signed dispatch plus gateway-to-V3 result callback roundtrip. The web contract test passed 10 checks, including fixed audit query encoding for `item_type=action&action_id=act-001&limit=1`.

This confirms:

- `GET /v1/external/integrations/{integration_id}/audit` can filter action records by exact `action_id`.
- The standalone panel can open a selected action from the audit timeline and render the redacted lifecycle/detail summary.
- The drilldown keeps using the same redacted action summary shape; it does not expose raw third-party callback result bodies or arbitrary message text.

## Action Permalink And Trace Export Follow-Up

Follow-up validated commit: `c751485`.

Local validation ran:

```text
node --test app/lib/external-integrations.test.mjs
npm run build
git diff --check
```

The deployment target pulled `c751485` and ran:

```text
node --test app/lib/external-integrations.test.mjs
npm run build
bash scripts/run-external-third-party-gateway-smoke.sh
```

Result: passed.

The web contract test passed 12 checks, including action drilldown permalink encoding and redacted operator trace export. The deployment smoke also generated a new readiness report with `ready: true`.

This confirms:

- the standalone panel can preserve `integration_id`, `audit_filter`, and `action_id` in the URL;
- operators can copy a direct action drilldown link;
- operators can export a redacted action trace JSON from the same audit summary;
- the external dispatch/result-callback smoke still passes after the panel change.
