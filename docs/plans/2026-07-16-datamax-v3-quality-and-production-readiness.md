# DataMax V3 Quality and Production Readiness Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use `@executing-plans` to implement this plan task-by-task.

**Goal:** 把当前已部署但尚未完全闭环的 DataMax V3，从“P0 安全候选版”推进到主线、数据库、字段语义、检索和真实问答证据一致的可发布版本。

**Architecture:** 保留当前 PostgreSQL system-of-record + PostgreSQL durable workflow queue + NATS wake-up + Rust workers + Next.js Web 的真实架构，不引入新的基础设施名词。先关闭连接串泄漏和发布线脱节，再建立可信数据库门禁与 migration runner，然后用持久字段契约授权结构化聚合、受控切换 PostgreSQL lexical 检索，并以不编排模型答案的真实 QA 收口。

**Tech Stack:** Rust 2021、Axum 0.8、SQLx 0.9、PostgreSQL 18.4、NATS、Next.js 16、React 19、Node.js ESM、GitHub Actions、systemd。

---

**Status:** READY FOR EXECUTION — NO TASK IN PROGRESS

**Created:** 2026-07-16

**Next task:** Task 1 — 连接串和启动日志脱敏

**Validation ledger:** `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

## 0. Frozen baseline

- Local release worktree: `codex/dataset-understanding-mvp` at `1edff9cdac585671f2ed64ed25690f2be121c750`, clean when this plan was cut.
- GitHub feature branch: `origin/codex/dataset-understanding-mvp` at the same SHA.
- GitHub main: `33e766a904bf28d63e71d170a463f33830c3cf86`; the release branch is 43 commits ahead and 0 behind.
- 8-server checkout: `/srv/aiv3/repo` at `1edff9cdac585671f2ed64ed25690f2be121c750`; 18 AIV3 services were active/running during the 2026-07-16 read-only inventory.
- PostgreSQL 18.4, NATS and the Web/API entry points were healthy; object storage remains the local filesystem.
- `ASSISTANT_RUN_SEMANTIC_SUPPLY_MODE=off`; dataset graph display and cross-dataset graph may remain enabled, but graph evidence does not influence answers.
- Asset parsing/import feature gates remain off.
- `RETRIEVAL_SEARCH_BACKEND` is unset in the live environment, so the assistant path defaults to `legacy_scan` although `postgres_lexical` is implemented.
- The last full platform API library report was 2,898 passed, 0 failed and 2 intentionally ignored, but 182 PostgreSQL bodies and 2 mock-gateway bodies returned early. It is not a complete integration pass.

Old plans are archived and non-executable. No old Task number, approval ID, provider window, feature flag scope or deployment baseline survives this cutover.

## 1. Planning decision

Three approaches were considered:

1. **Continue feature-first development.** Rejected because it expands the surface while the deployed branch lacks HEAD CI and tests can report green without executing their database bodies.
2. **Perform a big-bang platform rewrite.** Rejected because splitting the 250k-line API, moving object storage and introducing new infrastructure together would increase release risk without directly closing the proven answer-quality gaps.
3. **Staged hardening and quality closure.** Selected: security and mainline first, database trust second, semantic and retrieval quality third, real QA and release last.

## 2. Non-negotiable product and safety boundaries

- DataMax may select, normalize, authorize, rank and attribute evidence. It must not rewrite the user question or prescribe the model's answer wording, structure, conclusion, route or action.
- No answer template, dialogue planner, graph agent or hidden conclusion may be introduced by this plan.
- Graph-assisted answer supply stays off. A visual graph is not answer evidence unless a later independent plan proves a promotion candidate.
- Artifact creation, external actions and publication remain explicit-user-intent operations and fail closed.
- Suggested field semantics are display-only. Only a confirmed, scoped contract may authorize a non-count aggregate.
- Secrets, Oracle credentials, service names/SIDs, schemas and query permissions never enter Git, plan documents, validation ledgers or chat receipts.
- No business or pilot data deletion is part of this plan.
- Every task starts from `origin/main` after Task 2, uses a short-lived branch, runs its own gate, and is one reviewable commit unless the task explicitly calls for a docs-only receipt commit.
- A failed gate stops that task. Later tasks may not be used to excuse or mask it.

## 3. Dependency order

```text
Task 1 -> Task 2 -> Task 3 -> Task 4 -> Task 5 -> Task 6
                           \-> Task 7
Task 2 + Task 4 -----------------------> Task 8
Task 5 + Task 6 + Task 7 + Task 8 ----> Task 9 -> Task 10
```

Tasks 5 and 6 may not start before the migration runner and disposable PostgreSQL gate are green. Task 9 may not start while any P0 gate is unresolved.

## Task 1: Redact connection URLs from logs and errors

**Files:**

- Modify: `Cargo.toml`
- Modify: `crates/observability/Cargo.toml`
- Modify: `crates/observability/src/lib.rs`
- Modify: `crates/event-bus/Cargo.toml`
- Modify: `crates/event-bus/src/lib.rs`
- Modify: `crates/test-fixtures/Cargo.toml`
- Modify: `crates/test-fixtures/src/lib.rs`
- Modify: `crates/platform-api/src/external_database_source_config_support.rs`
- Modify: `crates/platform-api/src/main.rs`
- Modify: `crates/assistant-run-worker/src/main.rs`
- Modify: `crates/chat-session-worker/src/main.rs`
- Modify: `crates/dataset-output-worker/src/main.rs`
- Modify: `crates/report-planner-worker/src/main.rs`
- Modify: `crates/report-render-worker/src/main.rs`
- Modify: `crates/retrieval-worker/src/main.rs`
- Modify: `crates/retrieval-worker/src/bin/document-enrichment-worker.rs`
- Modify: `crates/memory-worker/src/main.rs`
- Modify: `crates/external-source-worker/src/main.rs`
- Modify: `crates/external-action-worker/src/main.rs`
- Modify: `crates/media-worker/src/main.rs`
- Create: `tools/check-sensitive-log-fields.mjs`
- Create: `tools/check-sensitive-log-fields.test.mjs`
- Modify: `package.json`

**Step 1: Write redaction tests first**

Add tests named:

- `connection_endpoint_redacts_userinfo_query_fragment_and_path`
- `connection_endpoint_handles_ipv6_and_default_ports`
- `connection_endpoint_rejects_malformed_or_secret_only_values`
- `event_bus_diagnostic_endpoint_never_contains_credentials`
- `fixture_failure_message_never_contains_database_credentials`

Inputs must include PostgreSQL and NATS URLs with usernames, passwords, query tokens, fragments, paths, percent-encoding and malformed values. Expected output may contain only scheme, host and port, or the literal `<redacted-endpoint>`.

**Step 2: Run the tests and prove the current code fails**

```powershell
cargo test -p observability
cargo test -p event-bus
cargo test -p test-fixtures
node --test tools/check-sensitive-log-fields.test.mjs
```

Expected: the new assertions fail because raw URLs are still emitted or interpolated.

**Step 3: Implement one shared helper**

Add the workspace `url` dependency and expose this contract from `observability`:

```rust
pub fn redact_connection_endpoint(raw: &str) -> String;
```

The function must remove username, password, path, query and fragment. Parse failures return `<redacted-endpoint>` and never echo the input. Reuse it from the database config wrapper instead of maintaining two policies.

**Step 4: Replace every raw startup field and error interpolation**

- Replace `%database_url` with `database_endpoint = %redact_connection_endpoint(&database_url)`.
- Replace `%nats_url` with `nats_endpoint = %redact_connection_endpoint(&nats_url)`.
- Fixture errors may include the disposable database name and safe endpoint, but not the original URL.
- The static scanner must reject `%database_url`, `%nats_url`, `{database_url}` in error strings and equivalent direct tracing fields.

**Step 5: Run the complete security gate**

```powershell
cargo fmt --all -- --check
cargo test -p observability
cargo test -p event-bus
cargo test -p test-fixtures
node --test tools/check-sensitive-log-fields.test.mjs
node tools/check-sensitive-log-fields.mjs
cargo check --workspace
rg -n "%database_url|%nats_url|\{database_url\}" crates
git diff --check
```

Expected: all tests pass; the final `rg` has no production log/error match.

**Step 6: Perform a count-only historical log audit**

On 8 server, search journal fields without printing matching lines or URLs. Record only service name, time range and match count. If any credential-bearing URL was historically logged, stop and rotate the affected secret through the approved secret channel; record rotation completion, never the secret.

**Step 7: Commit**

```powershell
git add Cargo.toml Cargo.lock crates package.json tools/check-sensitive-log-fields.mjs tools/check-sensitive-log-fields.test.mjs
git commit -m "fix: redact runtime connection endpoints"
```

**Completion standard:** no current code path or new journal line exposes raw PostgreSQL/NATS credentials; malformed URLs also fail closed.

## Task 2: Reconcile GitHub main, CI and the 8-server release line

**Files:**

- Modify: `.github/workflows/datamax-ci.yml`
- Create: `.github/workflows/datamax-release.yml`
- Create: `docs/operations/datamax-release-line.md`
- Modify: `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

**Step 1: Remove pull-request execution from the deployment host**

Move untrusted PR jobs from the `server8` self-hosted label to an isolated runner or `ubuntu-24.04`. Keep any 8-server action in a separate `workflow_dispatch` release workflow protected by a GitHub environment. The release workflow must check out an exact merged `main` SHA and may not accept an arbitrary ref.

**Step 2: Add the current P0 deterministic gates**

The main CI must run at least:

```bash
node tools/check-sensitive-log-fields.mjs
cargo test -p platform-api external_channel_action_prompt_support::tests --lib
cargo test -p platform-api database_field_support --lib
cargo test -p platform-api database_prompt_support --lib
cargo test -p external-source-connectors aggregate_query --lib
npm run smoke:newbai-customer-answer-live-capture -- --self-test
bash scripts/run-assistant-chat-contract-smoke.sh
bash scripts/run-external-direct-reply-smoke.sh
```

Do not broaden the push trigger to the old feature branch merely to create a green badge. The proof must be a PR and merged `main` run.

**Step 3: Validate workflow syntax and local gates**

```powershell
git diff --check
cargo fmt --all -- --check
npm run test:web
npm --prefix apps/web run build
cargo check --workspace
```

Expected: local gates are green and the workflow diff contains no secret values or server credentials.

**Step 4: Open the one-time reconciliation PR**

Push Task 1 plus this docs/CI cutover branch, open a PR into `main`, and require the no-credential, Rust and Web jobs. Review the 43-commit difference explicitly; do not force-push or rewrite the deployed history.

**Step 5: Merge and align the server**

After required checks pass, merge to `main`. Fast-forward `/srv/aiv3/repo` to the exact merged SHA and make its upstream `origin/main`. Rebuild/restart only services changed by Task 1. Do not change feature flags.

**Step 6: Verify identity and runtime**

```text
local main SHA == origin/main SHA == 8-server checkout SHA
server worktree == clean
healthz == 200
readyz == 200
both public Web entry points == 200
changed services == active/running, NRestarts == 0
```

**Step 7: Commit the release receipt**

```powershell
git add docs/operations/datamax-release-line.md docs/validation/2026-07-16-datamax-v3-quality-readiness.md
git commit -m "docs: record mainline reconciliation receipt"
```

**Completion standard:** deployed code, GitHub `main`, local `main` and required CI all identify the same SHA; future work starts from short-lived branches off main.

## Task 3: Make PostgreSQL and mock integration tests fail instead of silently skipping

**Files:**

- Modify: `crates/test-fixtures/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `.github/workflows/datamax-ci.yml`
- Modify: `scripts/run-retrieval-quality-smoke.sh`
- Create: `scripts/run-disposable-postgres-gate.sh`
- Modify: `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

**Step 1: Add failing required-fixture tests**

Introduce `AIV3_REQUIRE_TEST_DATABASE=1`. Tests must prove that an absent URL, unsafe database name, connection failure or migration failure becomes a test failure when required. Local default behavior may remain diagnostic/optional.

**Step 2: Replace the two environment-dependent mock tests**

Start an in-process one-shot Axum HTTP server on `127.0.0.1:0` inside the tests for dispatch and callback. Remove early returns based on `EXTERNAL_THIRD_PARTY_MOCK_DISPATCH_URL` and `EXTERNAL_THIRD_PARTY_MOCK_RESULT_HELPER_URL`.

**Step 3: Run focused tests and confirm they fail before implementation**

```powershell
$env:AIV3_REQUIRE_TEST_DATABASE='1'
$env:PLATFORM_DATABASE_URL='postgres://invalid:invalid@127.0.0.1:1/aiv3_ci_test'
cargo test -p test-fixtures -- --nocapture
cargo test -p platform-api external_action_dispatch_posts_to_external_mock_gateway_from_env --lib -- --nocapture
```

Expected: the database test fails explicitly; the old mock test demonstrates its dependency on external environment configuration.

**Step 4: Add a disposable PostgreSQL 18.4 gate**

The script must create a unique database name containing `_test`, export the required flag, run migrations/tests, and remove the database in a trap. It must reject any shared or production-looking database and must never print the full URL.

The CI `postgres-integration` job uses a `postgres:18.4` service with:

```yaml
env:
  POSTGRES_DB: aiv3_ci_test
  POSTGRES_USER: postgres
  POSTGRES_PASSWORD: postgres
  AIV3_REQUIRE_TEST_DATABASE: "1"
  PLATFORM_DATABASE_URL: postgres://postgres:postgres@127.0.0.1:5432/aiv3_ci_test
```

CI logs are test-only, but the redaction scanner still applies.

**Step 5: Run the full required database gate**

```bash
cargo test -p test-fixtures
cargo test -p storage --lib
cargo test -p platform-api --lib -- --nocapture --test-threads=1
cargo test -p ingest-worker
cargo test -p static-page-worker --lib
bash scripts/run-retrieval-quality-smoke.sh --require-postgres --baseline
```

Expected: no `skipping ... postgres` or `skipping ... mock gateway` line; all former 182 PostgreSQL and 2 mock bodies execute. The previous comparison point is 2,898 passed and 2 intentionally ignored, but added tests may increase the total.

**Step 6: Make it a required check and commit**

```powershell
git add crates/test-fixtures/src/lib.rs crates/platform-api/src/lib.rs .github/workflows/datamax-ci.yml scripts/run-retrieval-quality-smoke.sh scripts/run-disposable-postgres-gate.sh docs/validation/2026-07-16-datamax-v3-quality-readiness.md
git commit -m "test: require disposable postgres integration"
```

**Completion standard:** the required CI cannot report green when PostgreSQL or mock test bodies did not run, and the disposable database is gone after the job.

## Task 4: Add migration ledger, checksum and a session advisory lock

**Files:**

- Create: `crates/storage/src/migration_runner.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/Cargo.toml`
- Create: `crates/storage/tests/migration_runner.rs`
- Modify after Task 2 creates it: `docs/operations/datamax-release-line.md`
- Modify: `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

**Step 1: Write failing PostgreSQL tests**

Add:

- `migration_runner_records_each_version_once`
- `migration_runner_rejects_checksum_drift`
- `migration_runner_serializes_two_concurrent_startups`
- `migration_runner_bootstraps_legacy_database_then_becomes_noop`
- `failed_migration_does_not_leave_a_ledger_row`

**Step 2: Prove current behavior lacks the contract**

```bash
AIV3_REQUIRE_TEST_DATABASE=1 cargo test -p storage migration_runner -- --nocapture
```

Expected: tests fail because no ledger or checksum enforcement exists.

**Step 3: Implement the ledger bootstrap**

Create this table under a dedicated acquired connection:

```sql
create table if not exists aiv3_schema_migrations (
    version text primary key,
    description text not null,
    checksum text not null,
    applied_at timestamptz not null default now()
);
```

Use SHA-256 over the exact embedded migration SQL. Add `sha2.workspace = true` to storage.

**Step 4: Implement locking and transactions**

- Acquire one session-level PostgreSQL advisory lock before checking or applying any version.
- Keep the same physical connection until all versions are checked.
- For each missing version, execute SQL and insert its ledger row in one transaction.
- A recorded version with a different checksum fails closed before later migrations run.
- Always release the session lock; do not use a transaction advisory lock across multiple transactions.
- For the first governed run on an existing database, execute the existing idempotent SQL once under the lock and then record it. Do not blindly mark historical versions as applied.

**Step 5: Re-run on PostgreSQL 18.4**

```bash
AIV3_REQUIRE_TEST_DATABASE=1 cargo test -p storage migration_runner -- --nocapture
AIV3_REQUIRE_TEST_DATABASE=1 cargo test -p storage --lib -- --nocapture
cargo check --workspace
```

Expected: 19 rows after first run, zero new rows after the second, one serialized application under concurrency, and an explicit checksum error on drift.

**Step 6: Update the release sequence and commit**

The runbook must say: backup/preflight, run migration once, verify ledger, then restart application services. Runtime `migrate()` calls remain safe no-ops for already recorded versions during this cycle.

```powershell
git add crates/storage docs/operations/datamax-release-line.md docs/validation/2026-07-16-datamax-v3-quality-readiness.md
git commit -m "feat: govern postgres schema migrations"
```

**Completion standard:** empty, legacy, repeated and concurrent PostgreSQL 18.4 migrations have deterministic evidence; checksum drift cannot be ignored.

## Task 5: Persist scoped field semantic contracts

**Files:**

- Create: `crates/storage/migrations/0021_dataset_field_semantic_contract.sql`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/semantic_understanding.rs`
- Modify: `crates/platform-api/src/dataset_semantic_snapshot.rs`
- Create: `crates/platform-api/src/dataset_semantic_contract_support.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/assistant_run_database_field_support.rs`
- Modify: `crates/platform-api/src/assistant_run_database_prompt_support.rs`
- Modify: `crates/platform-api/src/assistant_run_database_summary_support.rs`
- Modify: `crates/external-source-connectors/src/mysql.rs`

**Step 1: Write contract tests first**

Add:

- `semantic_dictionary_contract_round_trips_and_resolves_by_scope`
- `semantic_contract_history_is_append_only`
- `semantic_snapshot_carries_confirmed_field_contract_without_secret_material`
- `aggregate_query_allows_sum_only_for_confirmed_additive_contract`
- `aggregate_query_rejects_non_additive_sum`
- `database_field_semantics_prefers_confirmed_contract_over_name_heuristic`
- tenant/source/object isolation and revoked-contract negative controls.

**Step 2: Define the persistent contract**

Extend `semantic_dictionary_entries` with constrained fields equivalent to:

```text
unit
additivity: additive | semi_additive | non_additive | unknown
allowed_aggregations: count | sum | avg | min | max
default_aggregation
contract_status: suggested | confirmed | revoked
contract_source
contract_evidence (redacted JSON object)
contract_version
```

Add an append-only revision table containing tenant, source scope, field, previous/new contract, actor and timestamp. Do not store source credentials or sample sensitive values.

**Step 3: Extend storage and snapshot contracts**

Update `NewSemanticDictionaryEntry`, `SemanticDictionaryEntry`, upsert/resolve/list/map SQL and the semantic snapshot projection. Contract revision or status changes must alter the semantic source fingerprint and make the affected snapshot stale.

**Step 4: Add tenant-scoped APIs**

Provide authenticated list/suggest/confirm/revoke endpoints. Only owner/operator authorization may confirm or revoke. Normal dataset viewers receive read-only safe projections. Every write appends an audit revision.

**Step 5: Keep suggestions non-authoritative**

`suggested` contracts may improve display labels but may not authorize a query. `confirmed + numeric value type + allowed aggregation` is required for SUM/AVG/MIN/MAX. COUNT follows its separate safe rule.

**Step 6: Run the database and domain gates**

```bash
AIV3_REQUIRE_TEST_DATABASE=1 cargo test -p storage semantic_dictionary --lib -- --nocapture
cargo test -p external-source-connectors aggregate_query --lib
cargo test -p platform-api database_field --lib
AIV3_REQUIRE_TEST_DATABASE=1 cargo test -p platform-api dataset_semantic --lib -- --nocapture
cargo check --workspace
```

Expected: uncontracted case 012 remains fail closed; a confirmed additive contract round-trips with unit, version and provenance; revocation immediately removes authority.

**Step 7: Commit**

```powershell
git add crates/storage crates/platform-api crates/external-source-connectors
git commit -m "feat: add scoped field semantic contracts"
```

**Completion standard:** field meaning is versioned, attributable and tenant/source/table/column scoped; no name heuristic can promote itself to confirmed authority.

## Task 6: Expose real processing details and field contract review in the dataset page

**Files:**

- Modify: `apps/web/app/lib/dataset-understanding-api.js`
- Modify: `apps/web/app/lib/dataset-understanding-api.test.mjs`
- Modify: `apps/web/app/lib/dataset-understanding-graph.js`
- Modify: `apps/web/app/lib/dataset-understanding-graph.test.mjs`
- Modify: `apps/web/app/components/DatasetUnderstandingGraph.js`
- Modify: `apps/web/app/components/WorkspaceDirectoryPanel.js`
- Modify: `apps/web/app/globals.css`
- Modify if API projection is incomplete: `crates/platform-api/src/dataset_semantic_understanding_support.rs`
- Modify after Task 5 creates it: `crates/platform-api/src/dataset_semantic_contract_support.rs`

**Step 1: Add failing Web fixtures**

Create ordinary document, database table and image fixtures. For each, assert that the right-side inspector can display:

- source material and status;
- parse/clean result and version;
- detected structure;
- extracted knowledge/facts;
- searchable/index coverage;
- semantic snapshot status and evidence;
- field unit, additivity, allowed aggregation, contract status and source.

Missing information must have an explicit reason rather than a fabricated completed state.

**Step 2: Normalize one safe view model**

Keep API details separate from graph node labels. No raw credential, private path, full sample record or secret value may enter the browser payload. Each row carries a source locator or an honest `unavailable_reason`.

**Step 3: Add review controls**

- Viewers: read-only.
- Owners/operators: suggest, confirm or revoke a field contract.
- Confirmation requires type, unit/additivity choice, allowed aggregations and evidence note.
- The UI must clearly distinguish “系统建议”“人工确认”“待解释”“已撤销”.

**Step 4: Preserve the accepted dataset-page layout**

Do not reintroduce the duplicate dataset list or asset-library module. The parsing/understanding graph remains the main visual; the right-side inspector supplies details.

**Step 5: Verify**

```powershell
node --test apps/web/app/lib/dataset-understanding-api.test.mjs apps/web/app/lib/dataset-understanding-graph.test.mjs
npm run test:web
npm --prefix apps/web run build
cargo test -p platform-api dataset_semantic_contract --lib
git diff --check
```

Expected: all three data types explain each real stage; permission controls pass; no secret/path regression; existing graph and page-layout tests remain green.

**Step 6: Commit**

```powershell
git add apps/web/app crates/platform-api/src/dataset_semantic_contract_support.rs crates/platform-api/src/dataset_semantic_understanding_support.rs
git commit -m "feat: expose dataset processing and field contracts"
```

**Completion standard:** a customer can inspect what entered, what was parsed, what structure/knowledge was extracted, what is searchable and which field semantics are confirmed, without mistaking suggestions for facts.

## Task 7: Validate and canary PostgreSQL lexical retrieval

**Files:**

- Modify: `crates/platform-api/src/retrieval_query_support.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify if query evidence requires it: `crates/storage/src/lib.rs`
- Modify: `scripts/run-retrieval-quality-smoke.sh`
- Create: `docs/operations/postgres-lexical-rollout.md`
- Modify: `docs/validation/retrieval-quality-smoke.md`
- Modify: `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

**Step 1: Freeze the comparison corpus**

Use the existing 30+ retrieval cases plus deep-old-chunk, Chinese phrase, selected-document scope, owner scope and hidden-document negative controls. The same query, TopK and ACL apply to both backends.

**Step 2: Run the existing candidate tests on disposable PostgreSQL**

```bash
AIV3_REQUIRE_TEST_DATABASE=1 cargo test -p platform-api retrieval_search_backend --lib -- --nocapture
AIV3_REQUIRE_TEST_DATABASE=1 cargo test -p platform-api postgres_lexical_retrieval_search_prefers_cjk_phrase_match --lib -- --nocapture
AIV3_REQUIRE_TEST_DATABASE=1 cargo test -p platform-api postgres_lexical_retrieval_search_recalls_deep_old_chunk_beyond_latest_window --lib -- --nocapture
bash scripts/run-retrieval-quality-smoke.sh --require-postgres --baseline
```

Expected: no skip, no permission leak, deep old content and Chinese phrase cases pass.

**Step 3: Add diagnostic shadow comparison if needed**

If offline evidence is insufficient, add `postgres_lexical_shadow`: return the unchanged legacy result to the model while computing a bounded candidate comparison. Persist/log only IDs, ranks, timing and reason codes; never duplicate evidence text or alter the prompt.

**Step 4: Verify index and latency**

Run `EXPLAIN (ANALYZE, BUFFERS)` on disposable representative data and prove the `0014_retrieval_lexical_index.sql` index is used. Record p50/p95 against a fixed fixture and set the release budget before canary.

**Step 5: Canary on 8 server**

After offline gates pass, set `RETRIEVAL_SEARCH_BACKEND=postgres_lexical` only in the platform API environment and restart only `aiv3-platform-api`. Keep graph supply off. Run the frozen QA subset and compare receipts.

Rollback is one configuration change to `legacy_scan` or removal of the variable plus one API restart.

**Step 6: Commit**

```powershell
git add crates/platform-api/src/retrieval_query_support.rs crates/platform-api/src/lib.rs crates/storage/src/lib.rs scripts/run-retrieval-quality-smoke.sh docs/operations/postgres-lexical-rollout.md docs/validation/retrieval-quality-smoke.md docs/validation/2026-07-16-datamax-v3-quality-readiness.md
git commit -m "feat: qualify postgres lexical retrieval"
```

**Completion standard:** candidate recall is no worse than baseline, deep/CJK cases improve or stay green, hidden evidence remains invisible, the intended index is used, and rollback is proven.

## Task 8: Add minimum production observability and operations evidence

**Files:**

- Modify: `crates/observability/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Create: `crates/platform-api/src/operator_runtime_health_support.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Create: `scripts/smoke/operator-runtime-health.mjs`
- Modify: `package.json`
- Create: `docs/operations/postgres-backup-restore.md`
- Modify: `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

**Step 1: Write bounded snapshot tests**

Add:

- `runtime_operational_snapshot_is_tenant_scoped_and_bounded`
- `operator_runtime_health_exposes_counts_without_payloads_or_secrets`
- `readyz_stays_constant_cost_when_backlog_is_large`
- smoke self-test that rejects prompts, source rows, tokens, connection URLs and private paths.

**Step 2: Add safe runtime identity**

Every service startup record includes service name, application version and release commit, using safe connection endpoint fields from Task 1.

**Step 3: Add an operator-only health projection**

Expose bounded counts/status for:

- migration ledger head/checksum state;
- configured retrieval backend;
- workflow queue depth, age, failed/dead tasks;
- semantic snapshot/link age and failures;
- recent provider success/failure/latency summaries;
- worker last-success/last-failure when available.

Do not place these queries in `/readyz`; readiness remains constant-cost and limited to critical dependencies.

**Step 4: Add smoke and operations gates**

```powershell
cargo test -p platform-api operator_runtime_health --lib
npm run smoke:operator-runtime-health -- --self-test
cargo check --workspace
```

On 8 server, fix the `libpq.so.5: no version information available` package/library mismatch, add a disk threshold, and restore one PostgreSQL backup into a disposable database. Never restore over the business database.

**Step 5: Commit**

```powershell
git add crates/observability crates/storage crates/platform-api scripts/smoke/operator-runtime-health.mjs package.json docs/operations/postgres-backup-restore.md docs/validation/2026-07-16-datamax-v3-quality-readiness.md
git commit -m "feat: expose safe runtime health evidence"
```

**Completion standard:** a QA receipt can identify code SHA, migration head, retrieval backend, field contract revision, provider/model and queue health without exposing business content or secrets; backup restore is demonstrated.

## Task 9: Run controlled real-provider answer-quality acceptance

**Files:**

- Modify: `fixtures/newbai-customer-answer/cases.jsonl`
- Modify: `scripts/smoke/newbai-customer-answer.mjs`
- Modify: `scripts/smoke/newbai-customer-answer-live-capture.mjs`
- Create: `docs/validation/2026-07-16-datamax-v3-real-answer-quality.md`
- Modify: `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

**Step 1: Freeze the acceptance matrix**

Retain the established NewBai cases and add:

- ordinary questions that must not trigger artifacts/actions;
- DCF and unrelated questions that must not receive database evidence;
- case 012 with no contract, confirmed additive contract and revoked contract;
- unit, time-grain and non-additive negative controls;
- deep/CJK retrieval cases;
- hidden-document and cross-tenant permission controls;
- claim-to-visible-source-locator checks.

**Step 2: Record immutable runtime identity before each run**

The receipt includes release SHA, provider name, model name, retrieval backend, migration head, semantic snapshot version, field contract revision and active feature flags. It contains no secret.

**Step 3: Run feature-off baseline first**

- `ASSISTANT_RUN_SEMANTIC_SUPPLY_MODE=off` stays unchanged.
- Do not reopen graph A/B/C or add answer templates.
- No artifact, report, external action or publication request is sent.
- Run one case at a time, single concurrency, with at most two provider attempts per case.

**Step 4: Run the contract/lexical candidate**

Only evidence selection and authorized database supplies may differ. The original user text and model system behavior remain unchanged.

**Step 5: Evaluate fail-closed**

Acceptance requires:

- zero unauthorized artifact/action/publication side effects;
- zero hidden/cross-tenant evidence;
- ordinary questions remain ordinary model answers;
- case 012 aggregates only under a confirmed compatible contract, with correct unit and aggregation;
- revocation restores safe refusal/no aggregate;
- every material business claim has a visible source locator or is clearly framed as model reasoning;
- evaluator process failure, malformed inner receipt or captured side effect makes the whole run fail;
- provider calls/retries do not increase without documented cause.

**Step 6: Record a decision**

Allowed outcomes are `promote_candidate`, `keep_feature_off_and_fix`, or `rollback_retrieval_backend`. Do not reinterpret partial evidence as a pass.

**Step 7: Commit receipts only**

```powershell
git add fixtures/newbai-customer-answer/cases.jsonl scripts/smoke/newbai-customer-answer.mjs scripts/smoke/newbai-customer-answer-live-capture.mjs docs/validation/2026-07-16-datamax-v3-real-answer-quality.md docs/validation/2026-07-16-datamax-v3-quality-readiness.md
git commit -m "test: record real answer quality acceptance"
```

**Completion standard:** real answer quality is proven on immutable inputs without dialogue/answer orchestration, and side effects plus evidence permissions remain fail closed.

## Task 10: Build, release and close the plan

**Files:**

- Modify: `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`
- Modify: `docs/plans/2026-07-16-datamax-v3-quality-and-production-readiness.md`
- Modify: `docs/plans/datamax-active-execution-plan.md`
- Create: `docs/validation/2026-07-16-datamax-v3-release-receipt.md`

**Step 1: Run the final local and CI gates**

```powershell
cargo fmt --all -- --check
cargo check --workspace
npm run test:web
npm --prefix apps/web run build
node tools/check-sensitive-log-fields.mjs
git diff --check
```

Run the required PostgreSQL 18.4 integration, migration, P0 answer safety, retrieval and real-provider gates from Tasks 3–9. No early-return test body is allowed.

**Step 2: Merge the exact candidate to main**

All required GitHub checks must be green on the exact merge SHA. Create a release tag only after the merge SHA is known and recorded.

**Step 3: Preflight 8 server**

- verify clean checkout and exact main SHA;
- capture safe configuration/feature-flag summary;
- create PostgreSQL and object-root backups;
- run migration preflight and verify ledger/checksum;
- prepare exact rollback SHA and configuration.

**Step 4: Deploy without blue/green**

The host currently carries no production load, so deploy directly. Build/restart only changed services. Keep graph answer supply and asset import/parser gates off unless this plan explicitly promoted a named flag; it does not authorize either.

**Step 5: Verify final identity and behavior**

Required evidence:

```text
origin/main SHA == release tag SHA == 8-server SHA
migration ledger == source migration list and checksums
required CI == green
PostgreSQL integration == no early returns
real QA == accepted decision
healthz/readyz/Web == 200
changed services == active/running, NRestarts == 0
new warning/error count == 0 or explicitly explained
rollback command == tested or dry-run verified
```

**Step 6: Close rather than append forever**

Mark this plan `COMPLETE`, record the immutable release receipt, archive the finished plan, and replace the active pointer with the next approved plan or `NO ACTIVE IMPLEMENTATION`. Do not append future feature work here.

**Step 7: Commit**

```powershell
git add docs/plans docs/validation/2026-07-16-datamax-v3-quality-readiness.md docs/validation/2026-07-16-datamax-v3-release-receipt.md
git commit -m "docs: close datamax quality readiness release"
```

**Completion standard:** code, GitHub main, server runtime, migration ledger and QA receipts all point to one release identity, with a tested rollback and no hidden unfinished gate.

## 4. Explicitly deferred follow-on queue

The following work is real, but is not executable under this plan. Each item requires a new dated plan after Task 10 closes:

1. Split the giant `platform-api` and remove Worker reverse dependencies without changing behavior.
2. Consolidate report/static-page/HTML/video-PPT artifact lifecycles and product entry points.
3. Decide local filesystem versus S3-compatible object storage, then design HA/restore around that decision.
4. Expand OpenAPI from its current stub and add API compatibility gates.
5. Oracle upstream integration through server 10. Network reachability is not database readiness; first obtain service name/SID, schema and permissions through the secret channel, then choose a dedicated connector/agent architecture.
6. Members, bots, docs-site and broader enterprise RBAC.

This cycle does not introduce Qdrant, MinIO, Neo4j, Redis dependencies, Kubernetes, a microservice rewrite, graph-answer promotion, asset-import enablement or Oracle live access.

## 5. Definition of done for the whole plan

- Task 1–10 each has an exact commit and validation receipt.
- `docs/plans` contains only the unique active pointer and the current plan while execution is open.
- Raw connection URLs cannot reach logs or error receipts.
- GitHub main, required CI and 8 server share one release SHA.
- PostgreSQL tests and mock tests cannot silently return early in required CI.
- Migration ledger, checksum and concurrency behavior are proven on PostgreSQL 18.4.
- Only a confirmed scoped field contract authorizes non-count aggregation.
- Dataset processing and semantic details are inspectable without fabricated states or secret data.
- PostgreSQL lexical retrieval meets quality, ACL, index and rollback gates.
- Runtime health and backup/restore evidence are available without business payloads.
- Real-provider QA improves evidence supply without orchestrating the model's answer.
- Graph-assisted answer supply remains off unless a future independent plan proves otherwise.
