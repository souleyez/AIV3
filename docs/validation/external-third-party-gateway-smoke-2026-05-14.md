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

## Readiness Report Manifest Check Follow-Up

Follow-up validated commit: `51c6329`.

The deployment target pulled `51c6329` and ran:

```text
npm run test:external-readiness
npm run test:external-handoff
npm run validate:external-handoff
bash scripts/run-external-third-party-gateway-smoke.sh
```

Result: passed.

The gateway smoke now passes the handoff manifest into the readiness report generator by default:

```text
Handoff manifest: /srv/aiv3/repo/docs/integrations/third-party-handoff.sample.json
```

The validated deployment run generated:

```text
target/external-third-party-readiness/external-third-party-readiness-20260514T022922Z.json
target/external-third-party-readiness/external-third-party-readiness-20260514T022922Z.md
```

The JSON report included `handoff_manifest_ready` and a redacted `handoff_manifest_summary`, with `ready_for_customer_sandbox: true`. This makes the deployment smoke prove both sides of the pre-live package: the signed action/callback roundtrip and the third-party handoff manifest.

## Handoff Package Builder Follow-Up

Follow-up validated commit: `86f6aca`.

The deployment target pulled `86f6aca` and ran:

```text
npm run test:external-handoff-package
npm run build:external-handoff-package -- --basename deployment-package-check --generatedAt 2026-05-14T00:00:00.000Z
npm --prefix target/external-third-party-handoff/deployment-package-check run validate:handoff
```

Result: passed.

The generated package was written to:

```text
target/external-third-party-handoff/deployment-package-check
```

The package contains 10 files, including the Chinese/English third-party guides, sample handoff manifest, handoff validator, mock gateway reference, V3 operator smoke reference, generated README files, generated package scripts, and `handoff-package-manifest.json`. The package manifest reported `ready: true`, `package_root: "."`, and no handoff validation errors or warnings. Running `validate:handoff` from inside the package also returned `ready_for_customer_sandbox: true`.

## Handoff Package Integrity Follow-Up

Follow-up validated commit: `b9b89e0`.

The deployment target pulled `b9b89e0` and ran:

```text
npm run test:external-handoff-package
npm run test:external-handoff-package-integrity
npm run build:external-handoff-package -- --basename deployment-package-integrity-check --generatedAt 2026-05-14T00:00:00.000Z
npm --prefix target/external-third-party-handoff/deployment-package-integrity-check run validate:handoff
npm --prefix target/external-third-party-handoff/deployment-package-integrity-check run validate:package
```

Result: passed.

The generated package now contains 11 files, including `tools/validate-external-handoff-package.mjs`. The package-internal `validate:package` report returned `package_ready: true`, `included_file_count: 11`, and passed all checks:

- package manifest present;
- package type valid;
- package root relative;
- included file paths stay inside the package;
- included file byte sizes and SHA256 hashes match;
- handoff manifest is ready for customer sandbox.

## Handoff Package Archive Follow-Up

Follow-up validated commit: `8ac660c`.

The deployment target pulled `8ac660c` and ran:

```text
npm run test:external-handoff-package
npm run build:external-handoff-package -- --basename deployment-package-archive-check --generatedAt 2026-05-14T00:00:00.000Z
test -s target/external-third-party-handoff/deployment-package-archive-check.tar.gz
test -s target/external-third-party-handoff/deployment-package-archive-check.tar.gz.sha256
npm --prefix target/external-third-party-handoff/deployment-package-archive-check run validate:handoff
npm --prefix target/external-third-party-handoff/deployment-package-archive-check run validate:package
```

Result: passed.

The package builder wrote:

```text
target/external-third-party-handoff/deployment-package-archive-check
target/external-third-party-handoff/deployment-package-archive-check.tar.gz
target/external-third-party-handoff/deployment-package-archive-check.tar.gz.sha256
```

The generated archive SHA256 was reported by the builder and persisted in the sidecar file. Package-internal `validate:handoff` and `validate:package` both remained ready after archive generation.

## Handoff Package Archive Validator Follow-Up

Follow-up validated commit: `37d02c1`.

The deployment target pulled `37d02c1` and ran:

```text
npm run test:external-handoff-archive
npm run build:external-handoff-package -- --basename deployment-archive-validator-check --generatedAt 2026-05-14T00:00:00.000Z
node tools/validate-external-handoff-archive.mjs --archive target/external-third-party-handoff/deployment-archive-validator-check.tar.gz
npm --prefix target/external-third-party-handoff/deployment-archive-validator-check run validate:handoff
npm --prefix target/external-third-party-handoff/deployment-archive-validator-check run validate:package
```

Result: passed.

The archive validator returned `archive_ready: true`, `entry_count: 12`, root `deployment-archive-validator-check`, and SHA256 `46c71a8a9047681ddf23122c259fdbecbe2a93090e87284f1c1722386ae4d70e`.

The validated checks covered archive presence, `.sha256` sidecar presence, sidecar digest match, gzip/tar parsing, path traversal safety, single package root, required entries, package manifest file integrity, and handoff manifest readiness.

## Package-Included Archive Validator Follow-Up

Follow-up validated commit: `e48b563`.

The deployment target pulled `e48b563` and ran:

```text
npm run test:external-handoff-package
npm run test:external-handoff-package-integrity
npm run test:external-handoff-archive
npm run build:external-handoff-package -- --basename deployment-package-archive-script-check --generatedAt 2026-05-14T00:00:00.000Z
npm --prefix target/external-third-party-handoff/deployment-package-archive-script-check run validate:handoff
npm --prefix target/external-third-party-handoff/deployment-package-archive-script-check run validate:package
npm --prefix target/external-third-party-handoff/deployment-package-archive-script-check run validate:archive
```

Result: passed.

The generated package reported `fileCount: 12`. Package-internal `validate:package` returned `package_ready: true` and `included_file_count: 12`.

Package-internal `validate:archive` inferred the sibling archive path from the package directory, returned `archive_ready: true`, `entry_count: 13`, root `deployment-package-archive-script-check`, and SHA256 `3b0426688224af2ba29443931d57d76059656049f0794118a58b462d28ebd730`.

The deployment target finished at `e48b563` with a clean `main...origin/main` status.

## Handoff Release Validator Follow-Up

Follow-up validated commit: `ab38d8e`.

The deployment target pulled `ab38d8e` and ran:

```text
npm run test:external-handoff-release
npm run test:external-handoff-package
npm run test:external-handoff-archive
npm run build:external-handoff-package -- --basename deployment-release-validator-check --generatedAt 2026-05-14T00:00:00.000Z
npm --prefix target/external-third-party-handoff/deployment-release-validator-check run validate:release
npm run validate:external-handoff-release -- --package target/external-third-party-handoff/deployment-release-validator-check
```

Result: passed.

The generated package reported `fileCount: 13`. Both the package-internal `validate:release` script and root `validate:external-handoff-release` returned `release_ready: true`.

The release report returned SHA256 `2eee082b9b92ee799c328d5c800285d59bb30ee0c88d6b0f13f90560ab99b84f`, archive root `deployment-release-validator-check`, archive `entry_count: 14`, `included_file_count: 13`, and no package/archive error codes.

The deployment target finished at `ab38d8e` with a clean `main...origin/main` status.

## Handoff Release Report Follow-Up

Follow-up validated commit: `bb7ea22`.

The deployment target pulled `bb7ea22` and ran:

```text
npm run test:external-handoff-package
npm run test:external-handoff-release
npm run build:external-handoff-package -- --basename deployment-release-report-check --generatedAt 2026-05-14T00:00:00.000Z
npm run validate:external-handoff-release -- --package target/external-third-party-handoff/deployment-release-report-check
test -s target/external-third-party-handoff/deployment-release-report-check.release.json
```

Result: passed.

The build output included `releaseReportPath`, `releaseReportSha256`, `releaseReady: true`, and `fileCount: 13`.

The generated release report returned `release_ready: true`, SHA256 `bc3265e5416f0e50eef62e3cdf7aa1fd8b5f3f428fed95dce2fd54f3d75f40aa`, `checks: 17`, `included_file_count: 13`, and archive `entry_count: 14`.

The deployment target finished at `bb7ea22` with a clean `main...origin/main` status.

## Handoff Release Markdown Follow-Up

Follow-up validated commit: `a29428a`.

The deployment target pulled `a29428a` and ran:

```text
npm run test:external-handoff-package
npm run test:external-handoff-release
npm run build:external-handoff-package -- --basename deployment-release-markdown-check --generatedAt 2026-05-14T00:00:00.000Z
npm run validate:external-handoff-release -- --package target/external-third-party-handoff/deployment-release-markdown-check
test -s target/external-third-party-handoff/deployment-release-markdown-check.release.md
grep -q 'Status: \*\*READY\*\*' target/external-third-party-handoff/deployment-release-markdown-check.release.md
grep -q 'Archive SHA256:' target/external-third-party-handoff/deployment-release-markdown-check.release.md
```

Result: passed.

The build output included `releaseMarkdownPath`, `releaseMarkdownSha256`, `releaseReportPath`, `releaseReportSha256`, `releaseReady: true`, and `fileCount: 13`.

The generated release report returned `release_ready: true`, SHA256 `fddb4eb6595fbd9954020aa5ff413d7cc3f91b190eacd47654c3d3c2994ddc7b`, `checks: 17`, `included_file_count: 13`, and archive `entry_count: 14`.

The deployment target finished at `a29428a` with a clean `main...origin/main` status.
