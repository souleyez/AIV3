# Static Page Image2 Data Publish

**Scope:** V3 static-page generation, data binding, quality repair, and generated-artifact publishing.

## Learned Workflow

The Xinbai static-page delivery showed that high-quality customer pages need a two-stage path:

```text
requirements -> GPT-Image2 visual contract -> read image/layout -> bind real data -> validate口径 -> publish artifact
```

Do not treat early static-page modules as a fixed editing canvas when the user asks for a polished customer-facing page. The prompt sent to Image2 is the design brief; the final HTML is produced after reading the image and binding real V3 data.

## Required Data Policy

For state modules such as KPIs, rankings, warning distributions, store opportunity pools, and brand/shop detail tables:

- If the source table is a snapshot table, use the latest `txdate` or equivalent snapshot date.
- Do not sum rows across repeated snapshots unless the user explicitly asks for cumulative period totals.
- Trend charts may use multiple dates and must be labeled as a date series.
- Unit rendering follows the raw value after口径 validation: show `万` below `1亿`, and `亿` only when the value is at least `100000000`.
- When customer action depends on a store/brand decision, include a drill-down table and source count.

## Xinbai Case

The first artifact summed 40 `bi_contract_warning` snapshots and inflated the opportunity amount. The corrected page used latest `txdate=2026-05-10`:

- source preview rows: 33,931
- latest snapshot rows: 862
- grouped store + shop/brand + brandcode rows: 851
- top opportunity: 上浦建店 2701.1万
- published URL prefix: `https://v3.elepcloud.com/generated-artifacts/`

## V3 Capability Shape

V3 should expose this as an advanced static-page workflow:

1. Confirm or generate the Image2 prompt text.
2. Queue Cloudflare Image2 `static-page-visual`.
3. Store the image preview as a visual contract.
4. Render final HTML only after real data is bound and口径 checks pass.
5. Publish to generated artifacts on 8服务器.
6. Return the public link and the口径 summary.

The current frontend prompt payload now carries production rules:

- `workflow=requirements_to_image2_then_image_to_html`
- `snapshotAggregationPolicy=latest_snapshot_for_state_modules`
- `trendAggregationPolicy=date_series_only_for_trends`
- `publishTarget=v3_generated_artifacts_on_8_server`
- `codexHostEscalation.defaultMode=plan_only_or_dry_run`

## Cloudflare Codex Feasibility

It is feasible, but it should be an execution extension rather than a new authority.

Recommended boundary:

```text
V3 Assistant/ReAct
  -> validates user intent, dataset scope, capability, and allowlist
  -> packages a bounded advanced static-page task
  -> queues Codex Host / Cloudflare Codex
  -> receives structured patch/artifact proposal
  -> V3 validates, publishes, and audits
```

Safe first phase:

- Allow capability `static_page_advanced_publish` only in `plan_only` or read-only inspection mode.
- Let the host return a proposed plan,口径 findings, and artifact diffs, not directly mutate production.
- Require human confirmation before write-capable edits or publishing.

Fixed-template phase:

- Promote advanced static-page work to the fixed template `static_page_image2_data_publish`.
- Allow Cloudflare Codex to create and publish a new generated artifact without per-task human confirmation when the package uses V3-selected scope, publish mode is `new_generated_artifact_only`, and the output includes a validated snapshot/date/unit口径 report.
- Continue requiring human confirmation for overwrite, stable customer URL replacement, source-code changes, scope/credential expansion, customer-channel sending, or uncertain口径.
- Enable write-capable artifact edits only in isolated workspaces.
- Store output as V3 artifacts, not raw Codex HTML.

Do not allow:

- user-controlled CLI flags
- provider keys or browser tokens in prompts
- direct database credentials in Codex task text
- unrestricted server filesystem write access
- Cloudflare Codex as the browser-facing API or dataset permission authority
