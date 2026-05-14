# scripts

Bootstrap, smoke, and developer automation scripts live here.

Current smoke entrypoints:

- `run-external-bot-third-party-smoke.ps1`: contract smoke for external bots, third-party knowledge, ACL filtering, adapters, customer-hosted chat events, signed third-party mock dispatch/result callback, external actions, and the observe-first panel.
- `run-external-third-party-gateway-smoke.sh`: Linux/deployment-target smoke that starts a standalone third-party mock gateway, validates signed outbound dispatch against it, verifies the result callback contract, fails if no external request reaches the gateway, and writes JSON/Markdown readiness reports under `target/external-third-party-readiness` by default.
- `run-jump-host-codex-shim-smoke.ps1`: jump-host smoke for the Codex host shim.
- `run-jump-host-video-deliverable-smoke.ps1`: jump-host smoke for video/PPT deliverable validation.

External third-party handoff checks:

- `npm run test:external-handoff`: unit coverage for the customer sandbox handoff manifest validator.
- `npm run validate:external-handoff`: validates `docs/integrations/third-party-handoff.sample.json` for URL safety, dispatch auth delivery notes, callback allowlist confirmation, document/ACL fixtures, operations contacts, and raw-secret exclusion.
