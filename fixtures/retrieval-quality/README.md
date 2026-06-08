# Retrieval Quality Fixtures

This fixture set defines the baseline cases for AIV3 retrieval supply quality.
It is intentionally small enough to run on every retrieval change while still
covering the failure modes that motivated the DB-side lexical retrieval plan.

## Scope

Required case groups:

- `deep_old_chunk`: relevant evidence is not in the latest evidence window.
- `selected_document_scope`: only the requested document range may be used.
- `owner_scope`: private evidence must not cross users.
- `cjk_phrase`: Chinese business terms and short phrases must be recalled.
- `database_topn`: numeric, TopN, and trend questions must prefer structured aggregate supply.
- `row_identity`: row-level completeness must not be claimed when collapse is present.
- `mixed_database_document`: database rows and parsed document evidence must be supplied together.

NewBai/report-regression cases are intentionally represented across
`database_topn`, `row_identity`, and `mixed_database_document` instead of a
separate category. They assert that retrieval supply uses structured database
evidence for numeric answers, uses document evidence for narrative/template
context, and does not trigger report page regeneration or HTML fallback.

## Command

```bash
bash scripts/run-retrieval-quality-smoke.sh --baseline
```

Reports are written to:

```text
target/retrieval-quality-smoke/
```

The baseline command validates fixture coverage and runs the current retrieval
contract tests unless `RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true` is set.
