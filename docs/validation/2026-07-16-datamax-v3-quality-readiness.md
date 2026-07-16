# DataMax V3 Quality and Production Readiness Validation Ledger

**Plan:** `docs/plans/2026-07-16-datamax-v3-quality-and-production-readiness.md`

**Status:** OPEN — TASK 1 IMPLEMENTATION COMPLETE; TASK 2 NEXT

**Created:** 2026-07-16

## Recording rules

- Append one bounded receipt per completed task; do not turn this file into a raw terminal transcript.
- Record exact commit SHA, command, pass/fail count, skipped/early-return count, environment class and known limitation.
- Never record credentials, full connection URLs, tokens, private source rows, prompts containing secrets or Oracle login metadata.
- A reported pass with an unexecuted body is not a pass. Required database and mock gates must report zero silent early returns.
- A failed inner evaluator, missing receipt, malformed receipt or captured side effect fails the outer gate.
- Server checks record safe endpoint names, service states, counts and SHAs only.

## Cutover baseline

| Item | Value | Evidence state |
|---|---|---|
| Local release worktree | `1edff9cdac585671f2ed64ed25690f2be121c750` | verified 2026-07-16 |
| GitHub feature branch | `origin/codex/dataset-understanding-mvp` at `1edff9cd` | verified 2026-07-16 |
| GitHub main | `33e766a904bf28d63e71d170a463f33830c3cf86` | verified 2026-07-16 |
| Feature branch divergence | 43 ahead / 0 behind main | verified 2026-07-16 |
| 8-server checkout | `1edff9cdac585671f2ed64ed25690f2be121c750` | read-only verified 2026-07-16 |
| 8-server AIV3 services | 18 active/running | read-only verified 2026-07-16 |
| PostgreSQL | 18.4 | read-only verified 2026-07-16 |
| Platform health/ready | green | read-only verified 2026-07-16 |
| Graph answer supply | `off` | read-only verified 2026-07-16 |
| Retrieval backend | unset, therefore `legacy_scan` | read-only verified 2026-07-16 |
| Full platform library report | 2,898 passed / 0 failed / 2 ignored | prior local receipt |
| PostgreSQL bodies that returned early | 182 | known gap, not integration evidence |
| Mock-gateway bodies that returned early | 2 | known gap, not integration evidence |

## Gate tracker

| Task | Gate | State | Required proof |
|---|---|---|---|
| 1 | Connection URL redaction | implementation complete; runtime closure in Task 2 | commit `6bc3ac45`; local gates green; count-only journal audit set `rotation_required=postgresql` |
| 2 | Main/CI/server identity | pending | one SHA across main, required CI and server |
| 3 | Disposable PostgreSQL and mock integration | pending | zero silent early returns; disposable DB removed |
| 4 | Migration runner | pending | ledger/checksum/concurrency/legacy/no-op tests |
| 5 | Field semantic contract | pending | confirmed scoped contract round-trip and fail-closed negatives |
| 6 | Dataset processing and contract UI | pending | document/database/image fixtures, permissions, safe payload |
| 7 | PostgreSQL lexical retrieval | pending | quality/ACL/index/latency/rollback evidence |
| 8 | Runtime observability and restore | pending | safe bounded health, libpq fix, disposable restore |
| 9 | Real-provider QA | pending | immutable runtime identity, claim evidence, zero side effects |
| 10 | Release closeout | pending | main/tag/server/migration/QA identity and rollback |

## Initial known blockers

1. Raw `database_url` and `nats_url` values are directly traced by multiple startup paths; Task 1 must run before new product work.
2. The deployed 43-commit release line has no CI run on its HEAD because it is not merged to main.
3. Required CI has no disposable PostgreSQL 18.4 job and cannot currently distinguish the 184 known early-return bodies from executed passes.
4. `storage::migrate()` replays embedded SQL without a ledger, checksum or session lock.
5. `semantic_dictionary_entries` does not yet persist unit, additivity, allowed aggregations or revision history.
6. Real answer-quality acceptance and claim-level traceability remain unproven.

## Receipts

### 2026-07-16 plan cutover receipt

- `docs/plans` contains exactly the active pointer and the new dated implementation plan.
- Nine dated plans and the previous active pointer were moved to `docs/archive/plans`.
- All 18 archived plan bodies have a first-line `ARCHIVED ... NON-EXECUTABLE` banner; the archive README defines the non-inheritance rule.
- Non-archive documentation has zero reference to the old `docs/plans/2026-07-13*`, `2026-07-14*` or `2026-07-15*` paths.
- `docs/validation/README.md` now uses repository-relative links and includes this ledger.
- `git diff --check` passed; line-ending notices were informational and no whitespace error was reported.

This was a docs-only plan cutover and archive operation; it did not claim a code, CI, database, provider or deployment result.

### 2026-07-16 Task 1 connection redaction receipt

**Implementation identity**

- Commit: `6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7` (`fix: redact runtime connection endpoints`).
- Environment: local Windows worktree, branch `codex/dataset-understanding-mvp`.
- Scope: shared fail-closed endpoint redaction, 12 startup log replacements, safe NATS connection diagnostics, safe disposable-database errors and a repository static scanner.
- `async_nats` dependency targets are denied by a mandatory subscriber filter composed with `RUST_LOG` using AND; verbose runtime filters cannot reopen the dependency URL logging path.

**Test-first and deterministic gate evidence**

| Gate | Result |
|---|---|
| Initial redaction test | failed to compile before `redact_connection_endpoint` existed |
| Initial scanner test | failed with `ERR_MODULE_NOT_FOUND` before the scanner existed |
| `cargo test -p observability` | 4 passed / 0 failed / 0 ignored |
| `cargo test -p event-bus` | 3 passed / 0 failed / 0 ignored |
| `cargo test -p test-fixtures` | 4 passed / 0 failed / 0 ignored |
| focused platform API redaction test | 1 passed / 0 failed / 2,899 filtered out |
| `node --test tools/check-sensitive-log-fields.test.mjs` | 5 passed / 0 failed |
| `npm run check:sensitive-log-fields` | passed; zero repository violation |
| `cargo check --workspace` | passed |
| `cargo fmt --all -- --check` | passed |
| raw-field `rg` gate | zero production match |
| `git diff --cached --check` | passed |

The scanner covers direct display/debug fields, sensitive field names, member expressions, named and positional formatting, multiline invocations, all three Rust macro delimiters, tracing events/spans and character literals. Its failure output contains only file, line and rule; it never prints the matching source text.

**8-server count-only retained-journal audit**

- Audit end: `2026-07-16T08:01:53+08:00` on 8 server.
- Audit range: all journal records retained on the host at audit time.
- Output boundary: service name, count and first/last timestamp only; no matching line or URL was printed.
- Credential-bearing PostgreSQL userinfo matches: 1,610 across 13 services.
- Credential-bearing NATS userinfo matches: 0 across all 18 AIV3 services.
- Connection URL secret-query matches: 0 across all 18 AIV3 services.

| Service | PostgreSQL userinfo count | First retained match | Last retained match |
|---|---:|---|---|
| `aiv3-assistant-run-worker.service` | 361 | 2026-05-21T09:40:25.731936+0800 | 2026-07-15T20:18:11.102483+0800 |
| `aiv3-chat-session-worker.service` | 332 | 2026-05-27T11:40:14.855079+0800 | 2026-07-10T18:16:03.182448+0800 |
| `aiv3-dataset-output-worker.service` | 8 | 2026-05-27T11:40:14.827660+0800 | 2026-07-10T15:31:45.125988+0800 |
| `aiv3-document-enrichment-worker.service` | 16 | 2026-06-06T13:55:40.438988+0800 | 2026-07-14T14:35:06.985858+0800 |
| `aiv3-external-action-worker.service` | 10 | 2026-05-27T11:40:14.836535+0800 | 2026-07-10T15:31:45.030601+0800 |
| `aiv3-external-source-worker.service` | 23 | 2026-05-21T19:40:24.437954+0800 | 2026-07-10T15:31:45.046100+0800 |
| `aiv3-ingest-worker.service` | 27 | 2026-05-21T22:02:47.806592+0800 | 2026-07-10T15:31:45.024570+0800 |
| `aiv3-media-worker.service` | 12 | 2026-05-27T11:40:14.843049+0800 | 2026-07-10T15:31:45.016431+0800 |
| `aiv3-memory-worker.service` | 7 | 2026-05-27T11:40:14.848186+0800 | 2026-07-10T15:31:44.997954+0800 |
| `aiv3-platform-api.service` | 762 | 2026-05-20T20:46:57.391425+0800 | 2026-07-15T20:18:11.362942+0800 |
| `aiv3-report-planner-worker.service` | 9 | 2026-05-27T11:40:14.859135+0800 | 2026-07-10T18:16:03.169074+0800 |
| `aiv3-report-render-worker.service` | 7 | 2026-05-27T11:40:14.868237+0800 | 2026-07-10T15:31:44.994114+0800 |
| `aiv3-retrieval-worker.service` | 36 | 2026-05-21T22:17:02.296090+0800 | 2026-07-14T14:35:06.923515+0800 |
| **Total** | **1,610** | **2026-05-20T20:46:57.391425+0800** | **2026-07-15T20:18:11.362942+0800** |

The other five AIV3 services had zero credential-bearing userinfo match: `aiv3-codex-host-agent`, `aiv3-codex-responses-shim`, `aiv3-dataset-semantic-link-worker`, `aiv3-static-page-worker` and `aiv3-web`. This audit sets `rotation_required=postgresql`. Rotation is intentionally deferred: Task 2 must first deploy commit `6bc3ac45`, verify new startup records contain only safe endpoints, rotate the PostgreSQL credential through the approved secret channel, restart affected services and re-run the count-only fresh-window gate. NATS rotation is not required by this audit.

**Known limitation and mutation boundary**

- Historical journal records remain on the host; no log deletion or journal vacuum was performed.
- No GitHub push, deployment, service restart, feature-flag change, provider call, database mutation or credential rotation occurred in Task 1.
- Production closure remains pending Task 2 because the deployed binaries are still the frozen baseline and cannot yet prove a fresh redacted journal window.
