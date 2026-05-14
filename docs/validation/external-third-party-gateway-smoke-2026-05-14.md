# External Third-Party Gateway Smoke - 2026-05-14

## Target

- Deployment target: `8服务器`
- Repository path: `/srv/aiv3/repo`
- Validated commit: `0cfb9c9`
- Gateway: `scripts/external-third-party-mock-gateway.mjs`
- Runner: `scripts/run-external-third-party-gateway-smoke.sh`
- Database environment: platform service environment from `/etc/aiv3/aiv3.env`

## Result

Status: passed.

The deployment target ran:

```bash
bash scripts/run-external-third-party-gateway-smoke.sh
```

The smoke started a standalone Node-based third-party mock gateway on `127.0.0.1:43180`, exported its dispatch URL into the platform test environment, ran the env-gated `platform-api` dispatch test, then queried the mock gateway request log to prove that an actual outbound HTTP dispatch reached the gateway.

## Passed Checks

```text
cargo test -p platform-api external_action_dispatch_posts_to_external_mock_gateway_from_env --lib -- --nocapture
```

Result: `1 passed; 0 failed`.

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

- V3 dispatches to an independently running third-party mock gateway, not only an in-process Rust listener.
- The gateway receives exactly one dispatch request.
- The dispatch request includes dispatch-specific Bearer authentication.
- `X-V3-Signature` validates against method, path/query, timestamp, nonce, and body hash.
- `X-V3-Content-SHA256` matches the request body.
- Raw prompt text, callback tokens, and third-party response secrets are not present in the gateway-safe request record or persisted V3 result summary.

## Notes

- This gateway smoke uses local HTTP on the deployment host. It validates the customer-gateway boundary shape and request signing semantics without requiring a public HTTPS sandbox certificate.
- The next layer should run the same contract against a real HTTPS customer sandbox or a packaged mock gateway reachable through the production ingress path.

## Readiness Report Follow-Up

Follow-up validated commit: `36b5c28`.

The deployment target pulled `36b5c28` and ran:

```text
npm run test:external-readiness
bash scripts/run-external-third-party-gateway-smoke.sh
```

Result: passed.

The gateway smoke now writes JSON and Markdown readiness reports under `target/external-third-party-readiness` by default. The validated deployment run generated:

```text
target/external-third-party-readiness/external-third-party-readiness-20260514T021006Z.json
target/external-third-party-readiness/external-third-party-readiness-20260514T021006Z.md
```

The readiness report marked the run `ready: true` and confirmed:

- signed dispatch reached the third-party mock gateway;
- dispatch Bearer, HMAC signature, and body hash all validated;
- requester summary was present;
- dispatch payload and callback response stayed redacted;
- result callback reached V3 and was accepted.

It also lists the remaining real third-party handoff items: public HTTPS endpoint, dispatch credentials, stable identifiers, callback allowlist/network rule, document/ACL fixtures, and operational escalation contact.

## Handoff Manifest Validator Follow-Up

Follow-up validated commit: `67327ab`.

The deployment target pulled `67327ab` and ran:

```text
npm run test:external-handoff
npm run validate:external-handoff
npm run test:external-readiness
bash scripts/run-external-third-party-gateway-smoke.sh
```

Result: passed.

The new manifest validator checked `docs/integrations/third-party-handoff.sample.json` and marked the handoff `ready_for_customer_sandbox: true`. The validated checks covered:

- manifest version and required object shape;
- customer technical contact;
- HTTPS URLs or explicitly approved loopback URLs;
- dispatch Bearer/HMAC auth delivery instructions;
- callback allowlist confirmation;
- document and ACL fixtures;
- operations contacts;
- absence of raw secret material.

The deployment run also regenerated a readiness package:

```text
target/external-third-party-readiness/external-third-party-readiness-20260514T022330Z.json
target/external-third-party-readiness/external-third-party-readiness-20260514T022330Z.md
```

The readiness report stayed `ready: true`, and the standalone gateway smoke again validated signed dispatch plus action-result callback delivery into V3.
