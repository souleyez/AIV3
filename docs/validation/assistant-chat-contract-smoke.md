# Assistant Chat Contract Smoke

This smoke validates the minimum user-facing chat contract before static-page quality work resumes.

The contract is:

- Plain ordinary chat remains a normal model conversation. V3 awareness is additive, not a capability limit.
- V3-only facts require supplied V3 evidence. When that evidence is absent, answers should mark the V3 fact boundary as currently invisible or unsupplied.
- ReAct invalid-action and step-limit paths must not surface observation JSON, execution trails, runtime manifests, tool traces, provider raw payloads, or similar internal observability fields as the final user answer.
- External-channel callbacks expose task/action/search status through public redacted fields only.
- Ordinary external chat events can return provider model-authored text when the AssistantRun runtime is configured, while placeholder deployments keep a safe acceptance reply.

## Command

```bash
bash scripts/run-assistant-chat-contract-smoke.sh
```

The script is non-destructive and does not load `/etc/aiv3/aiv3.env`. It writes machine-readable and Markdown reports under:

```text
target/assistant-chat-contract-smoke/
```

Run a separate deployment-target/provider smoke when real external-model credentials are intentionally enabled.

## Fixed Codex Task Audit Smoke

The fixed Cloudflare Codex escalation path has a separate PowerShell smoke:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly
```

It validates the server-owned fixed templates for static-page generation and answer-quality autofix. The local/default mode checks package shape, output validation, rejection behavior, exception notification intent, runtime diagnostics, and the no-confirm Image2 static-page pending/final reply contract without contacting the 8 server or sending customer-visible output.

For the advanced static-page chain specifically:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case static-page-no-confirm
```

This case asserts that customers can see the Image2 effect card while V3 continues to publish the generated page, with `effect_image_confirmation_required=false`, no public API field additions, and the final result delivered as the existing `artifact_link` reply shape.

Remote mode is guarded. Use `-BaseUrl https://v3.elepcloud.com -PlanOnly` for a health/readiness check. Server mutation cases require deployment review plus `-AllowServerMutation`; they must still create only new generated artifacts and must not deploy answer-quality patches automatically.

## 2026-05-25 Fixed Task No-Confirm Evidence

- Environment: local Windows workspace, plan-only, no customer-visible output
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case static-page-no-confirm -Json`
- Result: passed
- Checked commands:
  - `cargo test -p platform-api external_channel_static_page --lib`
  - `cargo test -p platform-api codex_host_fixed_task --lib`
  - `cargo test -p codex-host-agent static_page --lib`
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260525T034437Z.json`
- Customer confirmation: not required; the effect image is a viewable status card while V3 continues to publish
- Public response shape: unchanged; final success is exposed through the existing `artifact_link` reply

Read-only deployment readiness:

- Environment: `8服务器` public V3 endpoint, plan-only
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -PlanOnly -Case static-page-no-confirm -Json`
- Result: passed health check; mutation skipped by guard
- Health URL: `https://v3.elepcloud.com/healthz`

## 2026-05-17 Deployment Target Evidence

- Host: `8服务器`
- Repository: `/srv/aiv3/repo`
- HEAD: `8761d89`
- Command: `bash scripts/run-assistant-chat-contract-smoke.sh`
- Result: passed
- JSON report: `/srv/aiv3/repo/target/assistant-chat-contract-smoke/assistant-chat-contract-smoke-20260517T044056Z.json`
- Markdown summary: `/srv/aiv3/repo/target/assistant-chat-contract-smoke/assistant-chat-contract-smoke-20260517T044056Z.md`

The deployment target ran all eight user-facing chat contract checks successfully after a fast-forward pull from `c7f66fc` to `8761d89`.
