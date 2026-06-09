# NewBai Assistant Answer Smoke

This smoke validates the NewBai answer-quality handoff between retrieval ranking,
AssistantRun evidence supply, and model-facing provider input. It is narrower
than a live customer-answer evaluation: by default it does not call a real LLM
provider, does not enable `RETRIEVAL_SEARCH_BACKEND=postgres_lexical`, and does
not mutate deployment services.

## Command

```bash
bash scripts/run-newbai-assistant-answer-smoke.sh
```

Reports are written under:

```text
target/newbai-assistant-answer-smoke/
```

To require the Postgres-backed AssistantRun route to execute, point
`PLATFORM_DATABASE_URL` at a disposable fixture database whose name contains
`test`, `fixture`, `temp`, or `tmp`, then run:

```bash
NEWBAI_ASSISTANT_ANSWER_SMOKE_REQUIRE_DB=true bash scripts/run-newbai-assistant-answer-smoke.sh
```

Do not set `AIV3_ALLOW_TEST_FIXTURE_SHARED_DATABASE=1` against a shared or
production-like database. The fixture-backed test resets its database.

## Coverage

- Deterministic provider-input contract:
  - Calculation method and data-source questions must carry the sheet overview
    evidence into model-facing supply.
  - Month/day disambiguation must carry the February low-activity workbook, not
    the January workbook.
  - ASCII field questions must carry the combined `quekou` /
    `xuzengxiaoshou` field explanation.
  - Table-content questions must carry the sheet overview and warning-level
    logic, not a random row chunk.
- AssistantRun route contract:
  - Creates a local Postgres-backed NewBai fixture dataset.
  - Calls `create_assistant_run` in placeholder runtime mode.
  - Verifies the first `retrieval_evidence` supply item is the expected
    business evidence.
  - Verifies the user-visible placeholder answer does not expose raw supply
    payload fields.
- Retrieval ranking regression:
  - Re-runs the existing `retrieval_ranking` regression suite, including the
    NewBai sheet-summary, month/day, table-content, and ASCII-field cases.

## 2026-06-09 Local Disposable-DB Receipt

- Environment: local repo, disposable Postgres fixture database, default
  `legacy_scan`, no real provider call.
- Command:
  `NEWBAI_ASSISTANT_ANSWER_SMOKE_REQUIRE_DB=true bash scripts/run-newbai-assistant-answer-smoke.sh`
- HEAD: `d95cd61`
- JSON report:
  `target/newbai-assistant-answer-smoke/newbai-assistant-answer-smoke-20260609T010218Z.json`
- Markdown report:
  `target/newbai-assistant-answer-smoke/newbai-assistant-answer-smoke-20260609T010218Z.md`
- Command result: succeeded.
- Overall smoke status: passed.
- Provider input contract: passed.
- Retrieval ranking regression: passed, `10` tests.
- AssistantRun route validation: passed.
- DB fixture status: `executed`.

During this run a uniquely named disposable database
`ai_data_platform_v3_test_codex_p111_*` was created, used as
`PLATFORM_DATABASE_URL`, and dropped after the smoke. A follow-up read-only
check against `postgres.pg_database` returned no remaining
`ai_data_platform_v3_test_codex_p111_%` databases.

The earlier default local run correctly reported
`db_fixture_status=skipped_protected_database` because the default database name
was shared `ai_data_platform_v3`. That protected skip should still not be
treated as full AssistantRun route evidence.

## Remaining Evidence Needed

- Add a separate live/customer-answer quality receipt if real provider output is
  intentionally enabled.
- Run any deployment-target variant only against a non-customer fixture dataset,
  without enabling `postgres_lexical` and without calling a real provider unless
  explicitly approved.
