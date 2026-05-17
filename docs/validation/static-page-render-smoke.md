# Static Page Render Smoke

This smoke validates the generated static-page HTML artifact after the data-quality gate has passed.

The contract is:

- A representative static-page draft can render to a concrete `index.html` artifact.
- The final HTML contains the page shell, viewport metadata, module grid, mobile media rule, and all expected module sections.
- Deterministic chart fallback is real DOM/SVG output.
- Advanced ECharts modules expose only safe JSON hydration islands and do not inject remote scripts.
- Confirmed sample rows render without missing-data placeholders.
- The asset manifest records the renderer contract, zero attention modules for the fixture, and the expected export package files.

## Command

```bash
bash scripts/run-static-page-render-smoke.sh
```

The script is non-destructive and does not load `/etc/aiv3/aiv3.env`. It writes the generated HTML artifact plus machine-readable and Markdown reports under:

```text
target/static-page-render-smoke/
```

This smoke complements `run-static-page-quality-smoke.sh`: quality smoke checks whether a draft is allowed to proceed, while render smoke checks the generated HTML/manifest contract for a confirmed draft.
