# Static Page Browser Smoke

This smoke validates that a generated static-page HTML artifact opens correctly in a real browser viewport.

The contract is:

- The renderer can generate a representative `index.html` artifact.
- Chrome can open that artifact in desktop and mobile viewport sizes.
- All expected modules are visible, deterministic SVG chart fallbacks render, and the ECharts module exposes only a safe JSON hydration island.
- Confirmed fixture data does not render missing-data placeholders.
- The desktop and mobile layouts have no horizontal overflow, no module overlap, and no obvious text overflow.
- The mobile layout respects the generated module order, so the final page follows the draft's mobile ordering contract.
- The manifest preserves the direct-browser delivery contract: `index.html`, no remote scripts, deterministic chart fallback, and optional safe ECharts hydration.
- Screenshots are captured for review under the smoke report directory.

## Command

```bash
bash scripts/run-static-page-browser-smoke.sh
```

The script is non-destructive and does not load `/etc/aiv3/aiv3.env`. It writes generated HTML, screenshots, and JSON/Markdown reports under:

```text
target/static-page-browser-smoke/
```

If Chrome is not in a standard location, set `CHROME_BIN` to the browser executable before running the smoke.

This smoke complements `run-static-page-render-smoke.sh`: render smoke checks the generated HTML and manifest contract statically, while browser smoke opens the generated page and verifies the desktop/mobile presentation surface.
