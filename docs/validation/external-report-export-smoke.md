# External Report Export Smoke

This smoke validates the third-party report delivery contract after a static page/report request is recognized.

The contract is:

- The normal JSON endpoint can return a report card without suppressing the normal assistant answer.
- The SSE endpoint can publish the same report surface.
- The reply surface exposes a single customer report link through artifact/card fields; customer text must not repeat raw report URLs.
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

## 2026-06-05 8 Server Rollout Receipt

- Host: `8服务器`
- Public endpoint: `https://v3.elepcloud.com`
- Deployed commit: `eb10153a8`
- Service: `aiv3-platform-api.service`
- Service status after restart: `active`
- Build note: default server `cc` hit the known `aws-lc-sys` memcmp compiler guard; release build passed with `CC=clang CXX=clang++`.
- Dataset scope: `64fff6c8-10e2-4ee8-8243-23166cce3abc`
- Smoke command: `npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --dataset-external-ids 64fff6c8-10e2-4ee8-8243-23166cce3abc`
- Receipt: `target/external-report-export-smoke/20260605114601.json`

Results:

- JSON `/v1/external/channels/generic-chat-main/events`: passed.
- SSE `/v1/external/channels/generic-chat-main/events/stream`: passed.
- Report title: `新世界百货经营管理月报表`.
- Public URL: `https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html?focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A`.
- Public URL focus: `取高机会`.
- JSON response reported 2 artifact links because both card and top-level fields carry the same report surface; SSE reported 1 artifact link.
- Customer text URL repetition guard passed: JSON had one markdown/raw URL mention, SSE relied on artifact/card fields and did not repeat a text URL.
- Export fields present: `table_data_url`, `ppt_download_url`, `markdown_download_url`; `download_exports[]` contained 3 entries.
- Artifact files returned HTTP 200 with non-empty bodies: `index.html` 82810 bytes, `data.json` 327529 bytes, `data-snapshot.json` 327529 bytes, `table-data.csv` 27763 bytes, `report.ppt` 4675 bytes, `report.md` 2957 bytes.

## 2026-06-05 Xinbai Template Contract Receipt

The accepted Xinbai report template is now backed by a structural contract:

```text
docs/static-page-templates/xinbai-functional-modular-template-20260604/template-contract.json
```

Validation command:

```bash
npm run validate:xinbai-report-template
npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
```

Results:

- Local artifact validation: passed.
- Public 8-server artifact validation: passed.
- Required files checked: `index.html`, `data.json`, `data-snapshot.json`, `manifest.json`, `table-data.csv`, `report.ppt`, `report.md`.
- Required new modules checked: `店铺机会风险结构`, `租售比健康度`, `平均租售比`.
- Required data arrays checked: `storeList`, `opportunities`, `lowActivity`, `salesSeries`, `riskByStore`, and related report arrays.
- Public files checked: 7 non-empty files.

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
