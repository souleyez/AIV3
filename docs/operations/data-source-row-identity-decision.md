# DataMax Data-Source Row Identity Decision Memo

**Status:** draft decision record  
**Last updated:** 2026-06-06  
**Scope:** `hy-sql-traffic-area` / Xinbai operating-analysis database-source materialization  

This memo records the current row-identity decision boundary for database-source ingestion. It is intentionally sanitized: do not add source rows, credentials, database URLs, full table dumps, or customer document contents.

## Current Evidence

Latest 8-server read-only smoke receipts:

- `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T100024Z.json`
- `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T110226Z.json`
- `/srv/aiv3/repo/target/data-ingestion-staging-live-smoke/data-ingestion-staging-live-smoke-hy-sql-traffic-area-20260606T141410Z.json`

The current source-derived dataset is question/report ready, but two latest-sync tables collapse multiple source rows into fewer DataMax documents:

| Table | Configured identity columns | Source rows | Unique documents | Collapsed rows | Current interpretation |
| --- | --- | ---: | ---: | ---: | --- |
| `bi_contract_warning` | `parentcode`, `storecode`, `txdate` | 100 | 6 | 94 | Row-level report completeness is not proven. |
| `bi_rentsales_detail` | `storecode`, `contract_no`, `contract_startdate` | 100 | 1 | 99 | Row-level report completeness is not proven. |

Current-head smoke now distinguishes "composite configured but still insufficient" from "single-column identity missing a composite mapping":

- `bi_contract_warning`: current composite identity still collapses rows. Staging discriminator direction is contract/brand identifier, shop/storefront identifier, business-mode or metric type, and stable detail sequence if available.
- `bi_rentsales_detail`: current composite identity still collapses rows. Staging discriminator direction is period/statement date, brand or shop identifier, rent/sales detail type, and stable detail sequence if available.
- `bi_traffic_area`: latest sync has no source-row counts but existing documents are present, so freshness should be verified before using those documents for current reports.

Other latest-sync tables in the same audit were row-level or had no duplicate materialization signal:

- `bi_oa_zulinhetong`
- `bi_oa_zulinhetonggudingzujin`
- `bi_oa_zulinhetongtichengzujin`
- `nwstore`

## Decision Boundary

Default production stance:

- Keep current production mapping unchanged.
- Treat the two collapsed tables as entity/store/period-level evidence until row-level semantics are explicitly required and verified.
- Do not claim row-level completeness for reports that depend on all individual rows from `bi_contract_warning` or `bi_rentsales_detail`.
- Do not change public third-party API fields, auth, URLs, or response shape for this decision.

Business decision needed:

1. If business accepts entity-level or latest-snapshot semantics, keep the mapping and document the report口径 as aggregated/entity-level.
2. If business needs row-level detail, create a staging-only mapping with finer row discriminators and validate counts before any production mapping change.

## Staging-Only Row-Level Candidate

Do not apply this directly to production. Use it only in a staging dataset or a clearly named smoke dataset.

Candidate discriminator direction:

- `bi_contract_warning`: keep current columns and add a stable row discriminator if available, such as contract/brand/shop/business-mode/metric fields that distinguish rows under the same `parentcode + storecode + txdate`.
- `bi_rentsales_detail`: keep current columns and add a stable row discriminator if available, such as month/date/brand/shop/rent-type/detail-sequence fields that distinguish rows under the same `storecode + contract_no + contract_startdate`.

Required validation before production:

- Latest-sync `source_row_count`, `unique_document_count`, `unique_chunk_count`, and retrieval-evidence count are reported separately.
- Collapsed rows for the two target tables approach zero or have a documented intentional aggregation reason.
- Xinbai report totals do not inflate by summing repeated snapshots.
- Static-page report can still default to latest month/selected time range.
- No raw row values, credentials, database URLs, or full table dumps are printed in smoke output.

## Verification Commands

Read-only audit:

```bash
DATA_INGESTION_LIVE_SMOKE_DATABASE_URL="$PLATFORM_DATABASE_URL" \
DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY=hy-sql-traffic-area \
DATA_INGESTION_LIVE_SMOKE_API_BASE=http://127.0.0.1:3000 \
bash scripts/run-data-ingestion-staging-live-smoke.sh
```

If staging mapping code changes are made:

```powershell
cargo fmt --check -p ingest-worker -p retrieval-worker
cargo test -p ingest-worker --bin ingest-worker external_source_ingest_table_counts -- --nocapture
cargo test -p retrieval-worker --bin retrieval-worker external_index_document_ids -- --nocapture
cargo check -p ingest-worker -p retrieval-worker
```

Then rerun the read-only audit against the staging dataset/source.

## Rollback

For staging-only tests, rollback is to stop using the staging dataset/source mapping and continue using the current production mapping. Do not delete customer documents or source records as part of rollback.

## Current Open Items

- Confirm with business whether the two collapsed tables should represent row-level details or entity/latest-snapshot facts.
- If row-level is required, identify stable discriminator columns without inspecting or publishing raw rows in docs.
- Keep the current DataMax report pages honest about aggregation limits until staging row-level validation passes.
