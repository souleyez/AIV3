# scripts

Bootstrap, smoke, and developer automation scripts live here.

Current smoke entrypoints:

- `run-assistant-run-worker-smoke.sh`: non-destructive Linux/deployment-target smoke for the AssistantRun background completion worker queue, workflow registration, and redacted video completion dispatch contract.
- `run-external-bot-third-party-smoke.ps1`: contract smoke for external bots, third-party knowledge, ACL filtering, adapters, customer-hosted chat events, signed third-party mock dispatch/result callback, external actions, and the observe-first panel.
- `run-external-third-party-gateway-smoke.sh`: Linux/deployment-target smoke that starts a standalone third-party mock gateway, validates signed outbound dispatch against it, verifies the result callback contract, validates the handoff manifest, builds and validates a sendable handoff release package, runs the aggregate handoff gate inside the readiness report, fails if no external request reaches the gateway, and writes JSON/Markdown readiness reports under `target/external-third-party-readiness` by default.
- `run-jump-host-codex-shim-smoke.ps1`: jump-host smoke for the Codex host shim.
- `run-jump-host-video-deliverable-smoke.ps1`: jump-host smoke for video/PPT deliverable validation.

External third-party handoff checks:

- `npm run test:external-handoff`: unit coverage for the customer sandbox handoff manifest validator.
- `npm run test:external-readiness`: unit coverage for the third-party gateway readiness report generator, including optional handoff manifest and handoff release package summaries.
- `npm run validate:external-handoff`: validates `docs/integrations/third-party-handoff.sample.json` for URL safety, dispatch auth delivery notes, callback allowlist confirmation, document/ACL fixtures, operations contacts, and raw-secret exclusion.
- `npm run test:external-handoff-package`: unit coverage for the self-contained third-party handoff package builder.
- `npm run test:external-handoff-package-integrity`: unit coverage for the generated package integrity validator.
- `npm run test:external-handoff-archive`: unit coverage for the sendable `.tar.gz` archive and `.sha256` validator. To validate a generated archive directly, run `node tools/validate-external-handoff-archive.mjs --archive target/external-third-party-handoff/<package>.tar.gz`, or run `npm run validate:archive` from inside the generated package directory.
- `npm run test:external-handoff-delivery`: unit coverage for the receive-side delivery manifest validator that verifies the sibling package directory, archive, sidecar, release JSON, and release Markdown against `<package>.delivery-manifest.json`.
- `npm run test:external-handoff-all`: unit coverage for the one-command handoff validation report that combines handoff manifest, package, archive, delivery manifest, and release checks.
- `npm run test:external-handoff-release`: unit coverage for the combined release validator that checks the generated package directory, archive, sidecar, package manifest, archive root, delivery manifest, and handoff readiness together.
- `npm run validate:external-handoff-delivery -- --package target/external-third-party-handoff/<package>`: verifies the generated delivery manifest and all expected delivery artifacts before or after transfer.
- `npm run validate:external-handoff-all -- --package target/external-third-party-handoff/<package>`: emits one JSON report covering all customer handoff gates.
- `npm run validate:external-handoff-release -- --package target/external-third-party-handoff/<package>`: emits a ready/not-ready report for a generated handoff release before sending it to a third party.
- `npm run build:external-handoff-package`: writes a third-party handoff package under `target/external-third-party-handoff`, including public guides, the manifest sample, validation tooling, package-integrity/archive/delivery/release/all-in-one tooling, mock gateway reference, README files, a SHA256 package manifest, a `.tar.gz` archive, a matching `.sha256` sidecar, a sibling `.release.json` validation report, a sibling `.release.md` human-readable summary, and a sibling `.delivery-manifest.json` delivery artifact manifest for receive-side checksum verification.
