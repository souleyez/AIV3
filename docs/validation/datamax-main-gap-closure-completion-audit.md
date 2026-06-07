# DataMax Main Gap Closure Completion Audit

This audit maps the DataMax final definition of done to current evidence. The current active execution plan is `docs/plans/datamax-active-execution-plan.md`; older dated plan files have been consolidated and archived.

## Current State

- Date: 2026-06-06.
- Local/GitHub head before this audit document update: `7084aaf1a`.
- 8-server head before this audit document update: `7084aaf1a`.
- 8-server status: `## main...origin/main` plus the pre-existing untracked `mode` file, left untouched.
- Active 8-server services checked: `aiv3-platform-api.service`, `aiv3-web.service`, `aiv3-codex-host-agent.service`, `aiv3-document-enrichment-worker.service`, `aiv3-ingest-worker.service`, `aiv3-retrieval-worker.service`, and `aiv3-static-page-worker.service`.
- Code drift note: after the current-head release-gate code receipts through `ca26bcd`, later commits `fcd2401`, `f5ae5b8`, and `7084aaf` changed only plan/validation documentation. No service restart was required for these documentation-only commits.
- Current executable plan: `docs/plans/datamax-active-execution-plan.md`.

## Status Legend

- `Proven`: current evidence directly satisfies the requirement.
- `Proven with pending`: the plan explicitly allows this item to remain pending when credentials or approvals are unavailable, and the pending state is recorded without bypass.
- `Decision pending`: the technical safety boundary is proven, but a business or operator decision is still needed before the broader objective can be considered fully closed.

## Final Definition Audit

| # | Requirement | Status | Evidence | Remaining Work |
| ---: | --- | --- | --- | --- |
| 1 | Validation records final deployed commit and 8-server smoke receipts for current head. | Proven for deployed code; current doc head synced | `docs/validation/datamax-main-gap-closure.md` records release-gate receipts for deployed code through `ca26bcd`; local/GitHub/8 server are synchronized at `7084aaf`, with the final three commits documentation-only. | Re-run full release gate after behavior code changes, or if operator wants a fresh current-head mutation gate despite docs-only changes. |
| 2 | Third-party public URL/auth/request/response contract is unchanged. | Proven | Task 10 follow-up at `7084aaf` confirmed source/public copies match, online full/pure MD/HTML return `200`, compatibility `v3_*`/`X-V3-*` names remain stable, and no public URL/auth/required request/existing response field changed. | Continue re-auditing after any additive customer-visible artifact/report behavior. |
| 3 | Third-party ordinary 20-way, main-site 20-way, streaming, static-page 5-way, report/export, data-ingestion, and document-quality smokes pass or have explicit root-cause notes. | Proven with pending auth-only checks | Release gate receipts cover third-party 20-way, main-site scoped 20-way with dataset `31588c60-0885-47c4-81fe-4ff5c27de8e7`, main-site streaming, static-page 5-way, report/export, data-ingestion readiness, scoped documents, and document-quality. Authenticated model-gateway/operator checks have explicit credential root-cause notes. | Provide a legitimate operator credential to turn the remaining auth-dependent model-gateway smoke into passed. |
| 4 | Xinbai report returns one clickable primary link, exposes export files, preserves normal answer text, and uses modular monthly template by default. | Proven | Focus-link closure and report/export smoke at `0f72ca37fc1e` confirmed title `新世界百货经营管理月报表`, focus `取高机会`, one text report link, and `table-data.csv`, `report.ppt`, `report.md`. Template hygiene keeps `xinbai-functional-modular-template-20260604` as the primary default. | Keep focused report/export smoke in future release gates. |
| 5 | Temporary documents and dataset/document group scopes work across same third-party conversation and do not leak across conversations. | Proven | `external-scoped-document-chat` smoke passed dataset group plus explicit document union, same-conversation follow-up restore, changed-conversation isolation, and attachment-title scoped document answer. | Keep the smoke in future release gates. |
| 6 | Historical enrichment has safe summary-only dry-run receipt; real backfill disabled unless reviewed tiny batch is approved. | Proven | General fingerprint summary-only dry-run at `112cc82e8457`: `candidate_count=20`, `recorded_count=0`. One-dataset fingerprint summary-only dry-run on 2026-06-06 for dataset `cd024465-358e-458c-961d-a8894f2358c5`: `candidate_count=20`, `recorded_count=0`, `would_record_count=0`, `summary_only=true`. Fact-index guard rollout at `758be74ef7f5` blocked missing confirmation and broad dataset real-run attempts, then summary-only dry-ran 5 documents with `derived_fact_count=139`, `inserted_fact_count=0`, `snapshot_updated=false`. Enrichment precheck rollout at `30a164da2af2` blocked unsafe real enqueue shapes, then summary-only dry-ran 10 documents with `missing_fingerprint_count=10`, `would_enqueue_count=0`, `enqueued_count=0`. Fine-grained fingerprint resolution audit at `29470f6150ad` showed `skipped_reason_counts.file_not_found=20`. Reachable single-document precheck found document `00fc651b-99f7-444b-9ee7-59695b2736cf` with `missing_fingerprint_count=0`, `would_enqueue_count=2`, `enqueued_count=0`. | Real historical backfill remains disabled until a tiny reviewed batch is explicitly approved. A safe first candidate exists, but running it would still be a real historical enrichment enqueue and needs operator approval plus queue monitoring. |
| 7 | Authenticated model-gateway operator smoke passes with legitimate session, or remains clearly marked pending with no bypass. | Proven with pending | Current-head guard returned `401 auth_session_required`; 8-server env audit found no operator smoke cookie/email/local-key or main assistant streaming smoke cookie. No auth bypass, temporary allow-any, fabricated session, or role mutation was added. Task 9 follow-up reconfirmed the protected boundary. | Provide a legitimate operator cookie or email plus local-key login, then run `npm run smoke:model-gateway-operator`. |
| 8 | Data-source row identity semantics are documented for collapsed tables before production mapping change. | Proven with business decision boundary | `docs/operations/data-source-row-identity-decision.md` documents `bi_contract_warning` and `bi_rentsales_detail`; current live smoke shows question/report readiness plus 193 collapsed latest-sync rows. The recorded decision boundary is to keep production mapping unchanged and not claim row-level completeness unless staging discriminator validation proves it. | Business must decide whether row-level detail is required. If yes, test staging-only discriminator mapping before production. |
| 9 | Low-quality recovery remains passive and cannot block normal answers. | Proven | Current-head audit at `fc7c37048c76` shows hard gate absent, dedicated live-autofix flag absent, and `answer_quality_autofix` absent from task/capability allowlists. Local answer-quality regressions pass. | Keep disabled unless operator explicitly enables passive collection/manual review and the dedicated gates. |
| 10 | Template matching excludes stale Xinbai templates from normal customer-visible reuse. | Proven | Template hygiene tests and focused 8-server smoke confirm primary Xinbai modular template selection, old smoke/prewarm/fallback candidates skipped, and focus-bearing artifact links returned. | Do not delete old artifacts without explicit cleanup approval; keep runtime guards. |
| 11 | Model-visible platform capabilities route report/static-page/document/data/proactive tasks without tool traces or contract changes. | Proven | Capability routing rollout exposes internal capability catalog and selected 8-server smoke passed template-reference, temporary-contract/area, and traffic-stat report materials; ordinary guards emitted no artifact links. | Add customer phrasing fixtures when new misroutes are observed. |
| 12 | Online third-party integration docs match deployed additive contract and use DataMax naming. | Proven | Task 10 follow-up at `7084aaf` confirmed source/public MD/HTML copies match by SHA-256; public full/pure MD/HTML URLs returned `200`; DataMax naming, dataset scopes, artifact fields, report exports, SSE progress, and additive compatibility text were confirmed. | Rebuild/recheck docs after public contract additions. |
| 13 | No raw credentials, raw customer rows, full documents, provider payloads, or secret env names are recorded. | Proven for this audit trail | Each smoke/runbook receipt records sanitized command shapes and safety boundaries. Recent audits did not print bearer tokens, database URLs, raw rows, full documents, cookies, local keys, provider keys, or provider payloads. | Continue using sanitized reports; do not paste raw env or customer data into validation docs. |

## Remaining External Decisions

The engineering work in the executable plan is closed to the safe boundary recorded above. The following items remain intentionally outside automatic completion because they require credentials or operator/business approval:

- A legitimate operator credential is still required to convert model-gateway authenticated smoke from `pending` to `passed`.
- A business decision is still required for the two collapsed database-source tables: entity/latest-snapshot evidence vs row-level materialization.
- A real historical backfill batch remains intentionally disabled until an operator explicitly approves a tiny reviewed batch and rollback plan. Current fact-index tooling blocks accidental real runs without `--confirm-real-run` and rejects dataset-level real batches above explicit `--limit 5`; current enrichment enqueue tooling blocks missing confirmation, multi-kind dataset real enqueue, and broad dataset real enqueue. The earlier reviewed sample is blocked by `file_not_found=20`; a separate reachable single-document candidate now exists for approval, but no real enqueue has been run.

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

## Fresh Enrichment Precheck Receipt

8-server commit: `30a164da2af2`.

Guard checks:

- Missing `--confirm-real-run` was blocked.
- Dataset-level real enqueue with multiple kinds was blocked.
- Dataset-level real enqueue with `--limit 6` was blocked.

Summary-only dry-run:

```bash
./target/release/document-enrichment-backfill \
  --dataset-id cd024465-358e-458c-961d-a8894f2358c5 \
  --kind procedure_steps,table_structure \
  --limit 10 \
  --dry-run \
  --summary-only \
  --pretty
```

Result:

```json
{
  "document_count": 10,
  "missing_fingerprint_count": 10,
  "already_exists_count": 0,
  "would_enqueue_count": 0,
  "enqueued_count": 0
}
```

Safety notes:

- No historical enrichment runs were enqueued.
- The checked scope is not ready for enrichment enqueue until fingerprint coverage is proven.
- Summary-only output omitted per-document reports and titles.

## Fresh Fingerprint Resolution Reason Receipt

8-server commit: `29470f6150ad`.

Summary-only dry-run:

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
  "would_record_count": 0,
  "recorded_count": 0,
  "duplicate_count": 0,
  "skipped_count": 20,
  "action_counts": {
    "skipped": 20
  },
  "skipped_reason_counts": {
    "file_not_found": 20
  }
}
```

Safety notes:

- No historical fingerprint records were written.
- Summary-only output omitted per-document reports, titles, paths, URLs, and content.
- The current blocker is missing local object files for the reviewed sample, not remote object URLs or missing local-root configuration.

## Fresh Reachable Enrichment Candidate Receipt

8-server dry-run candidate:

```bash
./target/release/document-enrichment-backfill \
  --dataset-id 1bcf2529-0bbb-46e6-884f-c2b33db352c2 \
  --document-id 00fc651b-99f7-444b-9ee7-59695b2736cf \
  --kind procedure_steps,table_structure \
  --dry-run \
  --summary-only \
  --pretty
```

Result:

```json
{
  "document_count": 1,
  "missing_fingerprint_count": 0,
  "already_exists_count": 0,
  "would_enqueue_count": 2,
  "enqueued_count": 0,
  "enqueue_count_by_kind": {
    "procedure_steps_v1": 1,
    "table_structure_v1": 1
  }
}
```

Safety notes:

- No enrichment runs were enqueued.
- This is the current recommended tiny candidate if the operator approves a first real historical enrichment run.
