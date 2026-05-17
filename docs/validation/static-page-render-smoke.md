# Static Page Render Smoke

This smoke validates the generated static-page HTML artifact after the data-quality gate has passed.

The contract is:

- A representative static-page draft can render to a concrete `index.html` artifact.
- The final HTML contains the page shell, viewport metadata, module grid, mobile media rule, and all expected module sections.
- Deterministic chart fallback is real DOM/SVG output.
- Advanced ECharts modules expose only safe JSON hydration islands and do not inject remote scripts.
- Confirmed sample rows render without missing-data placeholders.
- The asset manifest records the renderer contract, zero attention modules for the fixture, expected export package files, and the browser delivery contract.
- The smoke artifact writes the declared handoff files, including `data-snapshot.json`, `data-quality-report.json`, `visual-bridge.json`, `runtime-requirements.json`, `render-spec.json`, and `README.md`.

## Command

```bash
bash scripts/run-static-page-render-smoke.sh
```

The script is non-destructive and does not load `/etc/aiv3/aiv3.env`. It writes the generated HTML artifact plus machine-readable and Markdown reports under:

```text
target/static-page-render-smoke/
```

This smoke complements `run-static-page-quality-smoke.sh`: quality smoke checks whether a draft is allowed to proceed, while render smoke checks the generated HTML/manifest contract for a confirmed draft.

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
