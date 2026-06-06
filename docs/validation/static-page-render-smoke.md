# Static Page Render Smoke

This smoke validates the generated static-page HTML artifact after the data-quality gate has passed.

The contract is:

- A representative static-page draft can render to a concrete `index.html` artifact.
- The final HTML contains the page shell, viewport metadata, module grid, mobile media rule, and all expected module sections.
- Deterministic chart fallback is real DOM/SVG output.
- Advanced ECharts modules expose only safe JSON hydration islands and do not inject remote scripts.
- Confirmed sample rows render without missing-data placeholders.
- The asset manifest records the renderer contract, zero attention modules for the fixture, expected export package files, and the browser delivery contract.
- The smoke artifact writes the declared handoff files, including `export-package.json`, `data-snapshot.json`, `data-quality-report.json`, `visual-bridge.json`, `runtime-requirements.json`, `render-spec.json`, and `README.md`.
- The same artifact also passes the standalone `validate-static-page-export-artifact` gate so generated delivery directories can be checked outside the smoke.

## Command

```bash
bash scripts/run-static-page-render-smoke.sh
```

The script is non-destructive and does not load `/etc/aiv3/aiv3.env`. It writes the generated HTML artifact plus machine-readable, Markdown, and standalone export-validation reports under:

```text
target/static-page-render-smoke/
```

This smoke complements `run-static-page-quality-smoke.sh`: quality smoke checks whether a draft is allowed to proceed, while render smoke checks the generated HTML/manifest contract for a confirmed draft.

## Fixed Codex Static-Page Escalation

The Image2-first advanced static-page path has an additional local fixed-template smoke:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case static-page-no-confirm
```

This smoke is non-destructive. It validates that the Image2-first static-page path exposes a no-confirm pending card, marks `effect_image_confirmation_required=false`, waits for preview-ready evidence before queuing Codex Host, accepts only new `/generated-artifacts/` static-page outputs, and converts the final publish event into an existing `artifact_link` reply. It does not publish or overwrite customer artifacts.

A guarded remote readiness check may be run after deployment:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -PlanOnly -Case static-page-no-confirm
```

Remote mutation remains operator-gated. Only run without `-PlanOnly` and with `-AllowServerMutation` after explicit deployment review on the approved host.

For a reviewed real third-party static-page smoke, provide a bearer token without committing it:

```powershell
$env:V3_EXTERNAL_CHANNEL_BEARER_TOKEN = "<private token>"
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 `
  -BaseUrl https://v3.elepcloud.com `
  -AllowServerMutation `
  -Case static-page-no-confirm `
  -ServerCaseConfigPath .\private\static-page-smoke.case.json
```

The optional private config can supply `connection_id`, `tenant_external_id`, `bot_external_id`, `available_document_source_id`, `available_document_external_ids`, `dataset_external_ids`, `requested_skills`, `template`, and `bearer_token_env`. If no bearer is configured, the script fails before mutation with `mutation_attempted=false`. Reports record only redacted counts/statuses and must not include the bearer value.

## 2026-06-06 Xinbai Template Library Hygiene

- Purpose:
  - keep `xinbai-functional-modular-template-20260604` as the only normal default template for Xinbai business-report matching;
  - prevent historical one-off, smoke, prewarm, and fallback static pages from being returned as customer-visible default report links;
  - keep public report card links available after internal field sanitization.
- Implementation:
  - dataset-overlap template matching now identifies Xinbai primary default candidates and chooses them ahead of generic accepted baselines;
  - Xinbai business-report context skips non-primary accepted baselines during normal overlap matching;
  - smoke/prewarm/test/fallback-like baselines are excluded from normal overlap matching;
  - public static-page reply card sanitization restores verified public `public_url`, `generated_artifact_url`, and `artifact_links` fields after internal fields are pruned.
- Sanitized accepted-template audit:
  - local retained generated template directory: `target/database-static-pages/xinbai-functional-modular-template-20260604`;
  - 8-server read-only accepted-template audit returned 60 recent accepted baselines, including primary modular Xinbai templates and historical one-off/prewarm/fallback/smoke artifacts;
  - audit output contained only draft ids, titles, scope hashes, default-prompt hashes, status, owner presence, public artifact URLs, and coarse tags.
- Local verification:
  - `cargo test -p platform-api static_page_template --lib` passed, 18 tests;
  - `cargo test -p platform-api external_channel_static_page_dataset_template_overlap --lib` passed, 2 tests;
  - `cargo test -p platform-api external_channel_static_page_reply --lib` passed, 14 tests;
  - `cargo test -p platform-api external_channel_static_page_artifact --lib` passed, 13 tests;
  - `cargo test -p platform-api external_channel_public_response --lib` passed, 2 tests;
  - `npm run validate:xinbai-report-template` passed;
  - `npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html` passed and checked 7 public files;
  - `cargo check -p platform-api` passed.
- Safety:
  - no old generated artifacts were deleted;
  - no third-party public URL, auth method, request field, or existing response field was changed;
  - no raw customer row, full customer document, source path, credential, bearer token, database URL, or provider payload was recorded.

## 2026-05-25 No-Confirm Static-Page Smoke Evidence

- Environment: local Windows workspace, plan-only, no server writes
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case static-page-no-confirm -Json`
- Result: passed
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260525T034437Z.json`
- Markdown summary: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260525T034437Z.md`
- Feature flags: local unit-level smoke; no `/etc/aiv3/aiv3.env` loaded
- Image2 job id: not created in local plan-only smoke
- Codex Host workflow id: not created in local plan-only smoke
- Final public URL: not published in local plan-only smoke
- Rollback: set `CODEX_HOST_TASK_ENABLED=false` and remove `static_page_image2_data_publish` from `CODEX_HOST_TASK_ALLOWLIST`

Read-only deployment readiness:

- Environment: `8服务器` public DataMax endpoint, plan-only, no mutation
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -PlanOnly -Case static-page-no-confirm -Json`
- Result: passed read-only readiness checks; static-page mutation case skipped by guard
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T021620Z.json`
- Checked public guide: `https://v3.elepcloud.com/external-integrations/pure-third-party-integration-guide.zh-CN.html`
- Checked external events auth guard: no-token `POST /v1/external/channels/generic-chat-main/events` returned `external_channel_auth_failed`
- Checked workflow queue diagnostics: `GET /v1/workflow-tasks/queue-stats` returned JSON
- Expected artifact URL prefix for reviewed mutation smoke: `https://v3.elepcloud.com/generated-artifacts/`

Guarded mutation dry run:

- Environment: `8服务器` public DataMax endpoint, no token configured
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -AllowServerMutation -Case static-page-no-confirm -Json`
- Result: failed by design before mutation; `mutation_attempted=false`, `bearer_configured=false`
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T043121Z.json`

Reviewed real mutation smoke:

- Environment: `8服务器` public DataMax endpoint, bearer loaded from the active `local-dev` external-channel connection without printing it
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -AllowServerMutation -Case static-page-no-confirm -ServerPollTimeoutSec 60 -ServerPollIntervalSec 10 -Json`
- Result: passed; `POST /events` returned `reply.reply_type=artifact_link`, `reply.task_status=static_page_published`, and a generated-artifact URL
- Assistant run: `e2d3c77f-f59c-46d0-90b7-2d1c979a42b0`
- Public URL: `https://v3.elepcloud.com/generated-artifacts/database-static-pages/external-channel/e2d3c77f-f59c-46d0-90b7-2d1c979a42b0/ec8ad76d-67e4-4766-9e74-6c90e607e0e2/index.html`
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T045233Z.json`

## 2026-05-17 Deployment Target Evidence

- Host: `8服务器`
- Repository: `/srv/aiv3/repo`
- HEAD: `7421232`
- Command: `bash scripts/run-static-page-render-smoke.sh`
- Result: passed
- Artifact directory: `/srv/aiv3/repo/target/static-page-render-smoke/static-page-render-smoke-20260517T045223Z-artifact`
- JSON report: `/srv/aiv3/repo/target/static-page-render-smoke/static-page-render-smoke-20260517T045223Z.json`
- Markdown summary: `/srv/aiv3/repo/target/static-page-render-smoke/static-page-render-smoke-20260517T045223Z.md`

The deployment target generated a representative `index.html` artifact and passed all generated HTML, DOM/SVG fallback, safe ECharts hydration, data placeholder, manifest, and export package checks after a fast-forward pull from `e9326b5` to `7421232`.
