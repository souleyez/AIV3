# Static Page Quality Smoke

This smoke validates the static-page data quality contract before effect-preview or final-render work.

The contract is:

- Static-page data snapshots carry field candidates from selected datasets, retrieval evidence, media windows, explicit module data, and document section-title hints.
- Chart modules must have renderable sample rows before effect-preview generation.
- Inferred evidence signals are not treated as confirmed chart data.
- Codex plan-only suggestions must repair weak static-page data quality before submitting preview generation.

## Command

```bash
bash scripts/run-static-page-quality-smoke.sh
```

The script is non-destructive and does not load `/etc/aiv3/aiv3.env`. It writes machine-readable and Markdown reports under:

```text
target/static-page-quality-smoke/
```

Visual/browser rendering inspection remains a separate smoke for generated HTML artifacts.
