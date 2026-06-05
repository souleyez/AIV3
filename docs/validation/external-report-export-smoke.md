# External Report Export Smoke

This smoke validates the third-party report delivery contract after a static page/report request is recognized.

The contract is:

- The normal JSON endpoint can return a report card without suppressing the normal assistant answer.
- The SSE endpoint can publish the same report surface.
- The customer-facing answer contains a single clickable report link rather than repeated raw URLs.
- The card title is `新世界百货经营管理月报表` by default.
- The default report prompt returns a report URL whose `focus` query value is `取高机会`; pass `--expected-focus <focus>` for another prompt or `--no-expected-focus` to disable this check.
- The card exposes `table_data_url`, `ppt_download_url`, `markdown_download_url` / `text_download_url`, and at least three `download_exports[]` entries.
- The published artifact directory serves `index.html`, `data.json`, `data-snapshot.json`, `table-data.csv`, `report.ppt`, and `report.md` with non-empty 200 responses.

## Command

```bash
npm run smoke:external-report-export -- \
  --base-url https://v3.elepcloud.com \
  --connection-id generic-chat-main \
  --bearer "$EXTERNAL_REPORT_EXPORT_SMOKE_BEARER" \
  --dataset-external-ids "$EXTERNAL_REPORT_EXPORT_SMOKE_DATASET_EXTERNAL_IDS"
```

The script writes a sanitized JSON receipt under:

```text
target/external-report-export-smoke/
```

It records only token presence, link/file checks, export counts, and compact SSE event metadata. It does not print or persist the bearer token.

## 2026-06-05 8 Server Manual Receipt

Before this reusable script was added, the same checks were run manually on `8服务器` after deploying commit `51e22fbbc`.

Validated endpoint: `https://v3.elepcloud.com`

Dataset scope: one Xinbai dataset group.

Results:

- JSON `/v1/external/channels/generic-chat-main/events`: passed.
- SSE `/v1/external/channels/generic-chat-main/events/stream`: passed.
- Report title: `新世界百货经营管理月报表`.
- Public URL: `https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html?focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A`.
- Public URL focus: `取高机会`.
- `download_exports[]`: 3 entries.
- Assistant answer report-link mentions: one.
- Artifact files checked: `index.html`, `data.json`, `data-snapshot.json`, `table-data.csv`, `report.ppt`, `report.md`; all returned HTTP 200 with non-empty bodies.

Rollback signal:

- Revert the latest report-card/export changes if third-party report replies lose the normal answer text, omit the card link/export fields, or repeat report URLs in customer-facing text.
