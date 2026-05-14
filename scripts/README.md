# scripts

Bootstrap, smoke, and developer automation scripts live here.

Current smoke entrypoints:

- `run-external-bot-third-party-smoke.ps1`: contract smoke for external bots, third-party knowledge, ACL filtering, adapters, customer-hosted chat events, signed third-party mock dispatch, external actions, and the observe-first panel.
- `run-external-third-party-gateway-smoke.sh`: Linux/deployment-target smoke that starts a standalone third-party mock gateway, validates signed outbound dispatch against it, and fails if no external request reaches the gateway.
- `run-jump-host-codex-shim-smoke.ps1`: jump-host smoke for the Codex host shim.
- `run-jump-host-video-deliverable-smoke.ps1`: jump-host smoke for video/PPT deliverable validation.
