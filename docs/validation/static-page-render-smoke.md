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
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case static_page_plan_only,static_page_new_artifact,static_page_overwrite_rejected
```

This smoke is non-destructive. It validates that V3 records `codex_host.fixed_task.queued`, accepts only new `/generated-artifacts/` static-page outputs, and rejects overwrite/stable-URL style outputs for human review. It does not publish or overwrite customer artifacts.

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
