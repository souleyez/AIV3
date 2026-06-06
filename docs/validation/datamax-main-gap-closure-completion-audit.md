# DataMax Main Gap Closure Completion Audit

This audit maps the final definition of done in `docs/plans/2026-06-06-datamax-main-gap-closure-plan.md` to current evidence.

## Current State

- Date: 2026-06-06.
- Local head before this audit document: `b8f578da5df4`.
- 8-server head before this audit document: `b8f578da5df4`.
- 8-server status: `## main...origin/main` plus the pre-existing untracked `mode` file, left untouched.
- Active 8-server services checked: `aiv3-platform-api.service`, `aiv3-web.service`, `aiv3-codex-host-agent.service`, `aiv3-document-enrichment-worker.service`, `aiv3-ingest-worker.service`, `aiv3-retrieval-worker.service`, and `aiv3-static-page-worker.service`.
- Code drift note: after deployed platform-api commit `0f72ca37fc1e`, later commits through `b8f578da5df4` changed only docs and the read-only data-ingestion smoke script. No new platform API binary behavior was introduced after the focused report/static-page smoke closure.

## Status Legend

- `Proven`: current evidence directly satisfies the requirement.
- `Proven with pending`: the plan explicitly allows this item to remain pending when credentials or approvals are unavailable, and the pending state is recorded without bypass.
- `Decision pending`: the technical safety boundary is proven, but a business or operator decision is still needed before the broader objective can be considered fully closed.

## Final Definition Audit

| # | Requirement | Status | Evidence | Remaining Work |
| ---: | --- | --- | --- | --- |
| 1 | Validation records final deployed commit and 8-server smoke receipts for current head. | Proven for deployed code; current doc head synced | `docs/validation/datamax-main-gap-closure.md` records current deployed code receipts, and 8 server is at `b8f578da5df4`. Commits after `0f72ca37fc1e` are docs/smoke-script only. | Re-run full release gate only after behavior code changes, or if operator wants a fresh current-head mutation gate despite docs-only changes. |
| 2 | Third-party public URL/auth/request/response contract is unchanged. | Proven | Task 10 re-audit confirmed source/public copies match, online docs return `200`, compatibility `v3_*`/`X-V3-*` names remain stable, and no public URL/auth/required request/existing response field changed. | Continue re-auditing after any additive customer-visible artifact/report behavior. |
| 3 | Third-party ordinary 20-way, main-site 20-way, streaming, static-page 5-way, report/export, data-ingestion, and document-quality smokes pass or have explicit root-cause notes. | Proven with pending | Release gate receipts cover third-party 20-way, main-site streaming, static-page 5-way, report/export, data-ingestion readiness, scoped documents, and document-quality. Main-site scoped 20-way and authenticated Cloudflare/model-gateway checks have explicit credential root-cause notes. | Provide a legitimate main-system/operator credential to turn pending auth-dependent checks into passed checks. |
| 4 | Xinbai report returns one clickable primary link, exposes export files, preserves normal answer text, and uses modular monthly template by default. | Proven | Focus-link closure and report/export smoke at `0f72ca37fc1e` confirmed title `新世界百货经营管理月报表`, focus `取高机会`, one text report link, and `table-data.csv`, `report.ppt`, `report.md`. Template hygiene keeps `xinbai-functional-modular-template-20260604` as the primary default. | Keep focused report/export smoke in future release gates. |
| 5 | Temporary documents and dataset/document group scopes work across same third-party conversation and do not leak across conversations. | Proven | `external-scoped-document-chat` smoke passed dataset group plus explicit document union, same-conversation follow-up restore, changed-conversation isolation, and attachment-title scoped document answer. | Keep the smoke in future release gates. |
| 6 | Historical enrichment has safe summary-only dry-run receipt; real backfill disabled unless reviewed tiny batch is approved. | Proven | General fingerprint summary-only dry-run at `112cc82e8457`: `candidate_count=20`, `recorded_count=0`. One-dataset fingerprint summary-only dry-run on 2026-06-06 for dataset `cd024465-358e-458c-961d-a8894f2358c5`: `candidate_count=20`, `recorded_count=0`, `would_record_count=0`, `summary_only=true`. Fact-index guard rollout at `758be74ef7f5` blocked missing confirmation and broad dataset real-run attempts, then summary-only dry-ran 5 documents with `derived_fact_count=139`, `inserted_fact_count=0`, `snapshot_updated=false`. | Real historical backfill remains disabled until a tiny reviewed batch is explicitly approved. Fingerprint/fact-index tools have dataset/document filters; fact-index real runs require `--confirm-real-run` and dataset-level `--limit <= 5`. |
| 7 | Authenticated model-gateway operator smoke passes with legitimate session, or remains clearly marked pending with no bypass. | Proven with pending | Current-head guard returned `401 auth_session_required`; 8-server env audit found no operator smoke cookie/email/local-key or main assistant streaming smoke cookie. No auth bypass, temporary allow-any, fabricated session, or role mutation was added. | Provide a legitimate operator cookie or email plus local-key login, then run `npm run smoke:model-gateway-operator`. |
| 8 | Data-source row identity semantics are documented for collapsed tables before production mapping change. | Decision pending | `docs/operations/data-source-row-identity-decision.md` documents `bi_contract_warning` and `bi_rentsales_detail`; latest live smoke at `7c2d92c5e8f7` shows question/report readiness plus 193 collapsed latest-sync rows, and Markdown/JSON recommended action says to verify composite identity in staging before relying on row-level reports. | Business must decide entity/latest-snapshot semantics vs row-level detail. If row-level is required, test staging-only discriminator mapping before production. |
| 9 | Low-quality recovery remains passive and cannot block normal answers. | Proven | Current-head audit at `fc7c37048c76` shows hard gate absent, dedicated live-autofix flag absent, and `answer_quality_autofix` absent from task/capability allowlists. Local answer-quality regressions pass. | Keep disabled unless operator explicitly enables passive collection/manual review and the dedicated gates. |
| 10 | Template matching excludes stale Xinbai templates from normal customer-visible reuse. | Proven | Template hygiene tests and focused 8-server smoke confirm primary Xinbai modular template selection, old smoke/prewarm/fallback candidates skipped, and focus-bearing artifact links returned. | Do not delete old artifacts without explicit cleanup approval; keep runtime guards. |
| 11 | Model-visible platform capabilities route report/static-page/document/data/proactive tasks without tool traces or contract changes. | Proven | Capability routing rollout exposes internal capability catalog and selected 8-server smoke passed template-reference, temporary-contract/area, and traffic-stat report materials; ordinary guards emitted no artifact links. | Add customer phrasing fixtures when new misroutes are observed. |
| 12 | Online third-party integration docs match deployed additive contract and use DataMax naming. | Proven | Source/public MD/HTML copies match by SHA-256; public full/pure MD/HTML URLs returned `200`; DataMax naming, dataset scopes, artifact fields, report exports, SSE progress, and additive compatibility text were confirmed. | Rebuild/recheck docs after public contract additions. |
| 13 | No raw credentials, raw customer rows, full documents, provider payloads, or secret env names are recorded. | Proven for this audit trail | Each smoke/runbook receipt records sanitized command shapes and safety boundaries. Recent audits did not print bearer tokens, database URLs, raw rows, full documents, cookies, local keys, provider keys, or provider payloads. | Continue using sanitized reports; do not paste raw env or customer data into validation docs. |

## Remaining Objective Gaps

These items prevent claiming the broader development objective is fully closed, even though the final definition has recorded safe pending states:

- A legitimate operator credential is still required to convert model-gateway authenticated smoke from `pending` to `passed`.
- A business decision is still required for the two collapsed database-source tables: entity/latest-snapshot evidence vs row-level materialization.
- A real historical backfill batch remains intentionally disabled until an operator explicitly approves a tiny reviewed batch and rollback plan. Current fact-index tooling now blocks accidental real runs without `--confirm-real-run` and rejects dataset-level real batches above explicit `--limit 5`.

## Fresh Backfill Dry-Run Receipt

Command shape on 8 server:

```bash
./target/release/document-fingerprint-backfill \
  --dataset-id cd024465-358e-458c-961d-a8894f2358c5 \
  --limit 20 \
  --dry-run \
  --summary-only \
  --pretty
```

Result:

```json
{
  "candidate_count": 20,
  "dataset_id": "cd024465-358e-458c-961d-a8894f2358c5",
  "document_id": null,
  "document_report_count": 20,
  "dry_run": true,
  "duplicate_count": 0,
  "include_existing": false,
  "limit": 20,
  "recorded_count": 0,
  "skipped_count": 20,
  "summary_only": true,
  "would_record_count": 0
}
```

Safety notes:

- No historical fingerprint records were written.
- No document titles, document bodies, external URLs, raw rows, database URLs, credentials, bearer tokens, provider payloads, cookies, or local keys were printed.
- Idempotent schema notices appeared because migrations are safe to re-run and objects already existed.

## Fresh Fact-Index Guard Receipt

8-server commit: `758be74ef7f5`.

Guard checks:

- Missing `--confirm-real-run` was blocked.
- Dataset-level real run with `--limit 6 --confirm-real-run` was blocked with the explicit `no greater than 5` guard.

Summary-only dry-run:

```bash
./target/release/fact-index-backfill \
  --dataset-id cd024465-358e-458c-961d-a8894f2358c5 \
  --limit 5 \
  --dry-run \
  --summary-only \
  --pretty
```

Result:

```json
{
  "document_count": 5,
  "derived_fact_count": 139,
  "inserted_fact_count": 0,
  "snapshot_updated": false,
  "summary_only": true
}
```

Safety notes:

- No historical fact records were written.
- No dataset snapshot was updated.
- Summary-only output omitted per-document reports and parse-quality warning details.
