# DataMax V3 Quality and Production Readiness Validation Ledger

**Plan:** `docs/plans/2026-07-16-datamax-v3-quality-and-production-readiness.md`

**Status:** OPEN — TASK 2 RECEIPT CANDIDATE; TASK 3 NOT STARTED

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
| 1 | Connection URL redaction | production-closed by Task 2 runtime | commit `6bc3ac45`; local gates green; runtime `ff60d0de`; fresh JSON journal gate zero |
| 2 | Main/CI/server identity | runtime complete; docs-only external receipt gate in progress | runtime `ff60d0de`; exact PR/main CI and runtime identity green; final receipt SHA/identity and runner cleanup remain external to this commit |
| 3 | Disposable PostgreSQL and mock integration | pending | zero silent early returns; disposable DB removed |
| 4 | Migration runner | pending | ledger/checksum/concurrency/legacy/no-op tests |
| 5 | Field semantic contract | pending | confirmed scoped contract round-trip and fail-closed negatives |
| 6 | Dataset processing and contract UI | pending | document/database/image fixtures, permissions, safe payload |
| 7 | PostgreSQL lexical retrieval | pending | quality/ACL/index/latency/rollback evidence |
| 8 | Runtime observability and restore | pending | safe bounded health, libpq fix, disposable restore |
| 9 | Real-provider QA | pending | immutable runtime identity, claim evidence, zero side effects |
| 10 | Release closeout | pending | main/tag/server/migration/QA identity and rollback |

## Initial known blockers at plan cutover

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

The other five AIV3 services had zero credential-bearing userinfo match: `aiv3-codex-host-agent`, `aiv3-codex-responses-shim`, `aiv3-dataset-semantic-link-worker`, `aiv3-static-page-worker` and `aiv3-web`. At Task 1 time this audit provisionally set `rotation_required=postgresql` and correctly deferred all mutation until redacted binaries were deployed. Task 2's current URL, role-verifier and HBA inspection supersedes that inference for the live platform role; see the Task 2 receipt. NATS rotation was not required by this audit.

**Known limitation and mutation boundary**

- Historical journal records were not intentionally deleted; no log deletion or journal vacuum was performed.
- No GitHub push, deployment, service restart, feature-flag change, provider call, database mutation or credential rotation occurred in Task 1.
- Task 1 production closure was subsequently supplied by Task 2 runtime `ff60d0deee81a2cbb98a70a4cbda9693962349d0` and its fresh JSON journal gate.

### 2026-07-16 Task 2 main/CI/8-server runtime receipt

**GitHub control plane and exact lineage**

- The repository is private and its current plan rejects branch protection, rulesets and protected-environment reviewers. The three jobs below were operator-enforced against exact SHAs; this ledger does not claim platform enforcement.
- Initial PR `#1` run `29462642453` ended before runner assignment because of GitHub account billing/spending-limit rejection. It was neither a pass nor a test failure.
- A temporary non-production Windows runner `aiv3-task2-isolated-20260716085742` was enabled only by `DATAMAX_CI_USE_ISOLATED_WINDOWS=true` and exact same-repository `DATAMAX_CI_TRUSTED_SHA`. It never used 8 server for PR code.

| Stage | Head or merge SHA | CI / identity run | Result |
|---|---|---|---|
| PR `#1` reconciliation | `5e420a375d12e9472b9af7b01b92327b88c684c6` | `29466636362` | `No-Credential Smoke`, `Rust P0`, `Web` success |
| PR `#1` merged main | `883f5f3e3d1367d1e7eb58968365f51c13b2ec5d` | `29467393919` | all three success |
| PR `#2` Node-path hotfix | `3631ce96ee1fb5862a9e4dea7dc0bfe9ce826d3a` | `29468730320` | all three success |
| PR `#2` merged main | `38c84d4a8b91dec0651b5bd0c64e1c18042a340b` | `29470149051` | all three success |
| PR `#3` ownership-boundary hotfix | `6412aef1654a7e75588af8d6ec3f31c031a1ac2d` | `29471314556` | all three success |
| Runtime main | `ff60d0deee81a2cbb98a70a4cbda9693962349d0` | `29472517847` | all three success |
| Runtime identity | `ff60d0deee81a2cbb98a70a4cbda9693962349d0` | `29473260743`, job `87540637627` | success; mutation none |

PRs `#1`, `#2` and `#3` merged at `2026-07-16T02:46:41Z`, `2026-07-16T03:56:32Z` and `2026-07-16T04:54:57Z`, respectively.

The exact runtime-main job ids were `87538371343` (`No-Credential Smoke`), `87538371349` (`Rust P0`) and `87538371336` (`Web`). Two earlier read-only identity runs failed safely with zero deployment mutation: `29468579036` exposed a missing non-interactive Node path, and `29471081803` exposed Git's dubious-ownership boundary. PRs `#2` and `#3` fixed those controls without changing global Git configuration or loosening the two root-only tracked fixture files. The workflow performs an ephemeral ancestry check; root SSH retains the authoritative full-worktree clean gate.

**8-server deployment**

- Pre-deploy SHA: `1edff9cdac585671f2ed64ed25690f2be121c750`.
- Runtime SHA: `ff60d0deee81a2cbb98a70a4cbda9693962349d0`.
- Frozen ancestor and redaction-floor checks: both true.
- Root deployment script SHA-256: `8B4247F36B3C24BB817E08E16F9CF95A571CABD73F82E3D22AB1E72E7FDD4BE5`.
- Release build: success from `2026-07-16T13:13:51+08:00` through `2026-07-16T13:23:34+08:00`; fourteen Cargo packages produced all sixteen Rust service binaries.
- The deployment helper's initial fresh-journal result is not accepted as evidence: the host `journalctl` rejected its ISO-8601 timestamp, while hidden stderr and `|| true` turned the producer failure into a zero count.
- Independent read-only corrected JSON window: host-local `2026-07-16 13:23:34` through `2026-07-16 13:24:04`; 808 actual records scanned.

| Service | Credential userinfo | Secret parameter |
|---|---:|---:|
| `aiv3-assistant-run-worker.service` | 0 | 0 |
| `aiv3-chat-session-worker.service` | 0 | 0 |
| `aiv3-codex-host-agent.service` | 0 | 0 |
| `aiv3-codex-responses-shim.service` | 0 | 0 |
| `aiv3-dataset-output-worker.service` | 0 | 0 |
| `aiv3-dataset-semantic-link-worker.service` | 0 | 0 |
| `aiv3-document-enrichment-worker.service` | 0 | 0 |
| `aiv3-external-action-worker.service` | 0 | 0 |
| `aiv3-external-source-worker.service` | 0 | 0 |
| `aiv3-ingest-worker.service` | 0 | 0 |
| `aiv3-media-worker.service` | 0 | 0 |
| `aiv3-memory-worker.service` | 0 | 0 |
| `aiv3-platform-api.service` | 0 | 0 |
| `aiv3-report-planner-worker.service` | 0 | 0 |
| `aiv3-report-render-worker.service` | 0 | 0 |
| `aiv3-retrieval-worker.service` | 0 | 0 |
| `aiv3-static-page-worker.service` | 0 | 0 |
| `aiv3-web.service` | 0 | 0 |
| **Total** | **0** | **0** |

- Extending the corrected scan through `13:35:12` still read 808 records and returned zero for both match classes. The responses shim independently had zero records and zero matches in both windows.
- Runtime state: 18 active, 18 running, 18 with `NRestarts=0`.
- HTTP gate: local health `200`, local ready `200`, public docs `200`, public V3 `200`.
- `/etc/aiv3/aiv3.env`: `root:root 600`.
- Feature flags: unchanged by root-only value comparison.
- Web was not rebuilt because `1edff9cd..ff60d0de` contains no Web source delta.
- No provider call, migration, database mutation, journal deletion or feature-flag change occurred.

**PostgreSQL classification and fail-closed fixture**

- PostgreSQL is `18.4`.
- The live environment has exactly one platform database URL for role/database `ai_platform_v3` / `ai_data_platform_v3`; it is loopback and has no password component.
- The login role's password verifier is `NULL`; loopback host HBA rules are `trust` while local socket rules are peer.
- Classification: `platform_rotation_not_applicable`. No platform password existed to rotate, and Task 2 did not introduce a secret that `trust` would not enforce.
- Three disposable-role fixture attempts returned `fixture_ok=false` because both candidate and intentionally wrong passwords authenticated under `trust`. All returned `cleanup_ok=true`.
- Independent cleanup postcheck: temporary role `0`, database `0`, environment directory `0`, state directory `0`.
- The prepared live rotation script was not run. No live role, environment file or service was changed by the fixture.
- HBA hardening is a separate production-readiness task requiring a tested old/new-password rejection path and rollback.

**Independent post-deploy read-only recheck**

- Connection path: `ssh -J windows-jump 8服务器`; no fetch, checkout or configuration write was performed.
- Server `HEAD` and its existing local `origin/main` tracking ref both equalled `ff60d0deee81a2cbb98a70a4cbda9693962349d0`; root full status, including untracked files, had zero entries.
- Both `1edff9cd` and `6bc3ac45` were ancestors; the exact eighteen-unit set had no difference and all state/HTTP checks remained green.
- PostgreSQL client and server were both `18.4` (`server_version_num=180004`); the expected role and database each existed exactly once, login was enabled and the verifier remained `NULL`.
- Rotation state/controller count, fixture role/database count, fixture environment/state directory count and feature-snapshot residue were all zero.
- This recheck used Git optional-lock suppression and an ephemeral safe-directory option; it did not mutate Git, code, configuration, database, services, secrets or rotation state.

The original retained-journal scan's 1,610 matches cannot currently be mapped to an active platform password. An exact rerun over all eighteen currently retained journals returned zero even though the journal still retains records from May 20 onward. Task 2 performed no Codex-driven journal deletion or vacuum, so the discrepancy remains unresolved. It would be unsafe to claim that those historical values were rotated. Any external-datasource credential implicated by future inventory must be identified and rotated through its approved owner channel in a separately scoped task.

**Docs-only closure boundary**

This receipt commit intentionally cannot contain its own merge SHA or the final identity run id. After its PR and merged-main three-job gates pass, root SSH must prove the runtime-to-receipt delta contains only the four approved documentation paths, fast-forward without build/restart, and recheck health and count-only journals. The final release-identity run then supplies immutable external evidence. Both temporary repository variables and the runner registration must be removed afterward. Task 3 is not executable until that full external gate closes; no third self-referential receipt PR is required.
