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

- Environment: `8服务器` public V3 endpoint, plan-only, no mutation
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -PlanOnly -Case static-page-no-confirm -Json`
- Result: passed health check; static-page mutation case skipped by guard
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260525T034451Z.json`
- Health URL: `https://v3.elepcloud.com/healthz`
- Expected artifact URL prefix for reviewed mutation smoke: `https://v3.elepcloud.com/generated-artifacts/`

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
