# Database Source Integration Implementation Plan

**Goal:** Add a safe database-source connector to V3 so an external MySQL database can be inspected, mapped, synchronized into an explicit V3 dataset, parsed, indexed, and then used through the existing dataset question-answering, report, template, and static-page workflows.

**Architecture:** Extend the existing `external_source_connections` and `external-source-worker` pipeline instead of adding database logic to chat or document parse endpoints. A database source is only a data-source input: each real sync must resolve an effective target V3 dataset from the sync request or a configured default dataset, then create/update normal dataset documents that flow through parse, retrieval, question-answering, report, template, and static-page chains. Store only redacted connection metadata and environment variable names in V3; keep raw database credentials in server-side environment or secret files. Add a main-system database source UI for connection testing, schema inspection, target-dataset binding, table-to-document mapping, sync launch, and sync observability.

**Tech Stack:** Rust, Axum, Tokio, SQLx MySQL, PostgreSQL storage, existing `external-source-worker`, Next.js main system data source page.

---

## Current Context

The current test MySQL connection was inspected read-only:

- Host/port is reachable.
- MySQL version is `8.0.24`.
- Visible database is `hy_sql`.
- Current `hy_sql` contains only one empty table `s(a varchar(255))`.
- No real business schema exists in the test copy yet.

This means the first implementation should support schema inspection and configurable mapping, but the final table mapping cannot be completed until the third party provides real tables or DDL.

## Scope

Included for MVP:
- MySQL database source only.
- Server-side credential binding by environment variable.
- Read-only schema inspection.
- Read-only table preview with row limit.
- Single-table and multi-table row-to-document mapping.
- Target dataset binding for every sync; schema inspection and table preview can run without a target dataset, but row sync cannot.
- Full sync and simple incremental sync by `updated_at`, numeric `id`, or explicit version column.
- Reuse existing external source workflow stages: metadata, content, ingest, index.
- Reuse existing dataset question-answering, report generation, template skill, and static-page evidence supply after database rows become indexed dataset documents.
- Data source UI for configuration, testing, schema view, mapping, sync, and status.

Excluded for MVP:
- Writing back to the third-party database.
- Arbitrary user-entered SQL.
- Cross-database joins.
- Automatic semantic inference of all business objects.
- CDC/binlog streaming.
- Direct vector indexing from MySQL without V3 document ingestion.
- Answering questions or generating reports directly from live MySQL rows without dataset ingestion and visibility checks.
- Storing raw database credentials in V3 PostgreSQL.

## Security Rules

- Never store raw database passwords in repository files, migration files, V3 PostgreSQL rows, logs, browser state, workflow events, or assistant messages.
- Database config stored in V3 may include only `connection_env`, host/port/database redacted summary, table allowlist, and mapping rules.
- Database-derived content may enter model context only after it has become visible V3 dataset documents, retrieval evidence, report evidence, or static-page evidence.
- The connector must run `START TRANSACTION READ ONLY` or equivalent before source reads when the server supports it.
- Only allow `SELECT` against whitelisted tables and columns.
- Do not accept arbitrary SQL in the first version.
- Enforce row limits, timeout limits, and max text field size.
- Prefer a dedicated read-only account in production, even if the copied test database currently uses a privileged account.

## Configuration Contract

External source connection:

```json
{
  "connector_kind": "mysql",
  "source_key": "third-party-hy-sql",
  "display_name": "第三方测试数据库",
  "sync_mode": "pull",
  "permission_mode": "none",
  "config_redacted": {
    "database_source": {
      "kind": "mysql",
      "host_redacted": "8.155.12.154",
      "port": 23306,
      "database": "hy_sql",
      "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
      "default_dataset_id": "018f0000-0000-7000-9000-000000000001",
      "table_count": 1
    }
  }
}
```

Sync request:

```json
{
  "sync_kind": "full",
  "dataset_id": "018f0000-0000-7000-9000-000000000001",
  "connector_context": {
    "mysql_source": {
      "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
      "database": "hy_sql",
      "timeout_ms": 10000,
      "row_limit": 1000,
      "tables": [
        {
          "table": "documents",
          "object_type": "document",
          "id_column": "id",
          "title_column": "title",
          "content_columns": ["content"],
          "content_type": "text/markdown",
          "updated_at_column": "updated_at",
          "revision_strategy": "updated_at_hash",
          "metadata_columns": ["category", "owner_id"]
        }
      ]
    }
  }
}
```

Generated V3 external document shape:

```json
{
  "document_external_id": "mysql:documents:123",
  "revision_external_id": "updated_at:2026-05-21T10:00:00Z",
  "title": "文档标题",
  "content_type": "text/markdown",
  "body": "正文内容",
  "metadata": {
    "source_table": "documents",
    "source_primary_key": "123",
    "category": "制度"
  }
}
```

Dataset binding contract:

- `CreateExternalSourceSyncRequest.dataset_id` is the preferred target dataset binding.
- A database source may also carry `config_redacted.database_source.default_dataset_id`; sync may use it when the request omits `dataset_id`.
- MySQL sync must fail with `target_dataset_required` when neither request nor source config resolves an effective visible dataset.
- The effective dataset id is written to the workflow execution, and `ingest-worker` remains responsible for creating/updating V3 documents under that dataset.
- `document_external_id` stays source-stable across datasets, but document identity is scoped by `(tenant_id, dataset_id, source_id, document_external_id)` so the same source row can be synced into different datasets when required.
- AssistantRun, report generation, template skills, and static-page planning must not receive live database rows; they only receive indexed chunks, detail evidence, report data, or artifact inputs supplied through selected/visible datasets.

---

### Task 1: Add Shared Database Source Connector Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/external-source-connectors/Cargo.toml`
- Create: `crates/external-source-connectors/src/lib.rs`
- Create: `crates/external-source-connectors/src/mysql.rs`
- Test: `crates/external-source-connectors/src/mysql.rs`

**Step 1: Write failing tests**

Add tests for:
- parsing a `mysql_source` config with `connection_env`;
- rejecting raw `password`, raw `url`, or `connection_string` fields in JSON config;
- validating table names and column names;
- accepting optional `default_dataset_id` only as a dataset binding hint, not as a credential or query selector;
- preserving only redacted connection summary.

Example test:

```rust
#[test]
fn mysql_source_config_rejects_raw_password() {
    let raw = serde_json::json!({
        "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
        "password": "secret",
        "database": "hy_sql",
        "tables": []
    });

    let error = MySqlSourceConfig::from_value(&raw)
        .expect_err("raw password should not be accepted");

    assert!(error.to_string().contains("raw database secret"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p external-source-connectors mysql_source_config_rejects_raw_password
```

Expected: fail because the crate and type do not exist.

**Step 3: Add crate and dependency**

Add workspace member:

```toml
"crates/external-source-connectors",
```

Add SQLx MySQL support in workspace dependencies:

```toml
sqlx = { version = "0.8.6", default-features = false, features = ["runtime-tokio-rustls", "postgres", "mysql", "uuid", "chrono", "json"] }
```

Create types:
- `MySqlSourceConfig`
- `MySqlTableMapping`
- `MySqlRevisionStrategy`
- `DatabaseSourceRedactedSummary`
- `DatabaseSourceError`

Validation:
- `connection_env` is required.
- `database` is required.
- `table` and column names must match `^[A-Za-z0-9_]+$`.
- no raw password/url/token fields accepted in config.

**Step 4: Run tests**

Run:

```bash
cargo test -p external-source-connectors
cargo check -p external-source-connectors
```

**Step 5: Commit**

```bash
git add Cargo.toml crates/external-source-connectors
git commit -m "Add database source connector config"
```

---

### Task 2: Add MySQL Schema Inspector

**Files:**
- Modify: `crates/external-source-connectors/src/mysql.rs`
- Test: `crates/external-source-connectors/src/mysql.rs`

**Step 1: Write failing tests**

Add pure tests for SQL builder helpers:
- information schema query uses bind parameter for schema;
- table preview query quotes whitelisted identifiers;
- unsafe table names are rejected before SQL generation.

**Step 2: Run failing tests**

Run:

```bash
cargo test -p external-source-connectors mysql_schema
```

Expected: fail because inspector helpers do not exist.

**Step 3: Implement inspector**

Add:
- `inspect_mysql_schema(config: &MySqlSourceConfig) -> Result<DatabaseSchemaSnapshot>`
- `test_mysql_connection(config: &MySqlSourceConfig) -> Result<DatabaseConnectionHealth>`
- `preview_mysql_table(config: &MySqlSourceConfig, table: &str, limit: u32) -> Result<TablePreview>`

The inspector should fetch:
- server version;
- current database;
- table list;
- columns;
- primary keys and indexes;
- approximate row counts;
- table comments;
- update time when available.

Do not fetch business row values in schema inspection.

**Step 4: Add optional live test guard**

Add ignored or env-gated tests that run only when:

```text
MYSQL_SOURCE_TEST_DATABASE_URL
MYSQL_SOURCE_TEST_DATABASE_NAME
MYSQL_SOURCE_TEST_ALLOW_LIVE=true
```

Guard must refuse non-test database names unless explicitly allowed.

**Step 5: Run tests**

Run:

```bash
cargo test -p external-source-connectors
```

Optional live smoke:

```bash
MYSQL_SOURCE_TEST_ALLOW_LIVE=true cargo test -p external-source-connectors live_mysql_schema_inspect -- --ignored
```

**Step 6: Commit**

```bash
git add crates/external-source-connectors/src/mysql.rs
git commit -m "Add MySQL schema inspector"
```

---

### Task 3: Add API Contracts and Routes

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/Cargo.toml`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Add tests for:
- database source test route rejects raw secrets;
- schema route returns redacted schema snapshot;
- preview route requires table allowlist;
- MySQL sync route rejects requests without `dataset_id` when the source has no `default_dataset_id`;
- MySQL sync route resolves `default_dataset_id` only after normal dataset visibility checks;
- disabled source returns `external_source_disabled`.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p platform-api database_source
```

Expected: fail because routes and contracts do not exist.

**Step 3: Add contracts**

Add:
- `TestDatabaseSourceConnectionRequest`
- `TestDatabaseSourceConnectionResponse`
- `InspectDatabaseSourceSchemaRequest`
- `InspectDatabaseSourceSchemaResponse`
- `PreviewDatabaseSourceTableRequest`
- `PreviewDatabaseSourceTableResponse`
- `DatabaseSourceTableView`
- `DatabaseSourceColumnView`
- `DatabaseSourceMappingView`
- `DatabaseSourceDatasetBindingView`

All response types must be secret-free.

**Step 4: Add routes**

Add main-system API routes:

```text
POST /v1/external/sources/{source_id}/database/test
POST /v1/external/sources/{source_id}/database/schema
POST /v1/external/sources/{source_id}/database/preview
```

Use normal authenticated V3 access, not external third-party bearer auth.

**Step 5: Implement route behavior**

Route behavior:
- load `external_source_connections` by `source_id`;
- ensure source is enabled;
- normalize request config;
- resolve effective target dataset for sync requests from request `dataset_id` first, then source `default_dataset_id`;
- verify the effective dataset is visible to the current user before enqueuing sync;
- call `external-source-connectors` MySQL helper;
- record a redacted audit event or update source health status;
- never store raw secrets.

**Step 6: Run tests**

Run:

```bash
cargo test -p platform-api database_source
cargo check -p platform-api
cargo test -p contracts
```

**Step 7: Commit**

```bash
git add crates/contracts/src/lib.rs crates/platform-api/Cargo.toml crates/platform-api/src/lib.rs
git commit -m "Add database source inspection API"
```

---

### Task 4: Add Main-System Database Source UI

**Files:**
- Create: `apps/web/app/lib/database-source.js`
- Create: `apps/web/app/lib/database-source.test.mjs`
- Create: `apps/web/app/components/DatabaseSourcePanel.js`
- Modify: `apps/web/app/components/WorkspaceDirectoryPanel.js`
- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/globals.css`

**Step 1: Write failing JS tests**

Add tests for:
- normalizing schema snapshot;
- hiding secret-looking fields;
- building mapping payload;
- building sync payload with an explicit `dataset_id`;
- showing `target_dataset_required` as a configuration problem, not a parsing failure;
- summarizing empty schema safely.

**Step 2: Run tests to verify they fail**

Run:

```bash
cd apps/web
node --test app/lib/database-source.test.mjs
```

Expected: fail because helper does not exist.

**Step 3: Implement client helpers**

Add:
- `normalizeDatabaseSchema`
- `normalizeDatabasePreview`
- `buildDatabaseMappingPayload`
- `buildDatabaseSyncPayload`
- `summarizeDatabaseSourceHealth`

Fetchers:
- `testDatabaseSourceConnection`
- `inspectDatabaseSourceSchema`
- `previewDatabaseSourceTable`
- `startDatabaseSourceSync`

Use `/api/v3/external/sources/{source_id}/database/...` proxy paths.

**Step 4: Implement UI panel**

In the main "数据源" page, add a database source area:
- connection binding field: `connection_env`;
- target dataset selector and optional default dataset binding;
- create/select dataset affordance for database sync;
- host/port/database redacted display;
- test connection button;
- schema inspect button;
- table list with row counts and updated time;
- column list;
- mapping form:
  - object type;
  - table;
  - id column;
  - title column;
  - content columns;
  - updated/version column;
  - metadata columns;
- preview button;
- start sync button;
- last sync status.
- a readiness hint that database rows become usable for Q&A/report/templates only after the target dataset has synced, parsed, and indexed.

Do not add a visible raw password input in MVP.

**Step 5: Run tests/build**

Run:

```bash
cd apps/web
node --test app/lib/database-source.test.mjs
npm run build
```

**Step 6: Commit**

```bash
git add apps/web/app/lib/database-source.js apps/web/app/lib/database-source.test.mjs apps/web/app/components/DatabaseSourcePanel.js apps/web/app/components/WorkspaceDirectoryPanel.js apps/web/app/HomePageClient.js apps/web/app/globals.css
git commit -m "Add database source UI"
```

---

### Task 5: Add MySQL Source Fetcher to External Source Worker

**Files:**
- Modify: `crates/external-source-worker/Cargo.toml`
- Modify: `crates/external-source-worker/src/main.rs`
- Test: `crates/external-source-worker/src/main.rs`

**Step 1: Write failing tests**

Add tests for:
- `connector_fixture_for` recognizes `mysql_source`;
- a row maps to external document shape;
- the mapper refuses to emit sync content when workflow `dataset_id` is missing;
- missing id column fails clearly;
- incremental checkpoint filters generated query;
- content output contains `external_documents`.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p external-source-worker mysql_source
```

Expected: fail because MySQL source branch does not exist.

**Step 3: Add dependency**

Add:

```toml
external-source-connectors = { path = "../external-source-connectors" }
```

**Step 4: Implement connector branch**

In `connector_fixture_for`, detect:
- `mysql_source`
- `mysqlSource`
- `database_source`
- `databaseSource`

Fetch scopes:
- `Users`: return empty arrays for MVP unless a user mapping is explicitly configured.
- `Acl`: return empty ACL snapshots for MVP unless ACL mapping is configured.
- `Metadata`: fetch mapped rows without body.
- `Content`: fetch mapped rows with body.

Generated connector output:

```json
{
  "platform": "mysql",
  "users": [],
  "groups": [],
  "departments": [],
  "roles": [],
  "documents": [],
  "acl_snapshots": [],
  "connector_summary": {
    "kind": "mysql_source",
    "database": "hy_sql",
    "document_count": 0
  }
}
```

**Step 5: Add row-to-document mapper**

Mapping logic:
- `document_external_id = mysql:{table}:{id}`;
- `title` from title column, fallback to `{table} #{id}`;
- `body` joins configured content columns with section headers;
- `revision_external_id` from version column, updated_at, or hash of mapped content;
- `metadata` includes table, primary key, and configured metadata columns.
- dataset membership is not encoded in the row document payload; it comes from the workflow execution `dataset_id` so the same database row can be synced into different V3 datasets.

**Step 6: Add incremental checkpoint**

Support checkpoint fields:

```json
{
  "tables": {
    "documents": {
      "updated_after": "2026-05-21T10:00:00Z",
      "last_id": 123
    }
  }
}
```

For MVP, output next checkpoint in workflow output but do not mutate source DB.

**Step 7: Run tests**

Run:

```bash
cargo test -p external-source-worker mysql_source
cargo test -p external-source-worker
cargo check -p external-source-worker
```

**Step 8: Commit**

```bash
git add crates/external-source-worker/Cargo.toml crates/external-source-worker/src/main.rs
git commit -m "Add MySQL external source fetcher"
```

---

### Task 6: Wire Database Sync Into Existing Workflow

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/workflow-definitions/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`
- Test: `crates/workflow-definitions/src/lib.rs`

**Step 1: Write failing tests**

Add tests that enqueue a MySQL source sync request and assert:
- MySQL sync without request `dataset_id` and without source default dataset fails with `target_dataset_required`;
- MySQL sync with a source default dataset resolves that dataset through the same visibility check used by explicit `dataset_id`;
- workflow starts at `sync_users`;
- task context includes `connector_kind=mysql`;
- workflow execution carries the effective `dataset_id`;
- metadata/content stages receive `mysql_source` connector context;
- ingest and index stages remain unchanged.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p platform-api mysql_external_source_sync
cargo test -p workflow-definitions external_source_sync
```

**Step 3: Normalize connector context**

Allow `CreateExternalSourceSyncRequest.connector_context.mysql_source` through validation while still rejecting raw secrets.

Rules:
- `connection_env` allowed;
- raw `password`, `url`, `connection_string`, `token` rejected;
- table mapping required for sync;
- effective target dataset required for MySQL sync;
- empty mapping allowed only for schema inspection, not sync.

**Step 4: Preserve existing behavior**

Do not change HTTP source or mock connector behavior.

**Step 5: Run tests**

Run:

```bash
cargo test -p platform-api external_source_sync
cargo test -p workflow-definitions external_source_sync
cargo check -p platform-api
```

**Step 6: Commit**

```bash
git add crates/platform-api/src/lib.rs crates/workflow-definitions/src/lib.rs
git commit -m "Wire MySQL source sync into workflow"
```

---

### Task 7: Add Observability and Drift Checks

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/components/DatabaseSourcePanel.js`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing tests**

Add tests for:
- sync counts include `table_count`, `row_count`, `document_count`, `skipped_row_count`, `failed_row_count`;
- schema drift detects missing table or missing mapped column;
- source health moves to failed without exposing secrets.

**Step 2: Implement status fields**

Extend redacted status summaries with:
- target dataset id/title/indexing readiness;
- last schema inspect time;
- mapped table count;
- row count;
- document count;
- skipped rows;
- failed rows;
- last checkpoint;
- schema drift summary.

**Step 3: Show in UI**

In the database source panel, display:
- last sync status;
- last success/failure time;
- last error code;
- latest checkpoint;
- drift warnings.

**Step 4: Run tests/build**

Run:

```bash
cargo test -p platform-api database_source
cd apps/web
node --test app/lib/database-source.test.mjs
npm run build
```

**Step 5: Commit**

```bash
git add crates/contracts/src/lib.rs crates/platform-api/src/lib.rs apps/web/app/components/DatabaseSourcePanel.js apps/web/app/lib/database-source.js apps/web/app/lib/database-source.test.mjs
git commit -m "Add database source observability"
```

---

### Task 8: Update Documentation

**Files:**
- Create: `docs/integrations/database-source-integration.zh-CN.md`
- Create: `docs/integrations/database-source-integration.zh-CN.html`
- Modify: `apps/web/app/lib/external-integrations.js`
- Modify: `apps/web/public/external-integrations/third-party-integration-api.zh-CN.md`
- Modify: `apps/web/public/external-integrations/third-party-integration-api.zh-CN.html`

**Step 1: Draft documentation**

Include:
- MySQL source purpose;
- credential handling;
- required read-only account;
- connection env example;
- target dataset binding and the rule that Q&A/reporting read from datasets, not live database rows;
- table mapping example;
- full sync example;
- incremental sync example;
- error codes;
- security restrictions.

**Step 2: Ensure all fields have comments**

Every JSON field in examples must have a field table with comments.

**Step 3: Build/validate docs**

Use the existing doc generation path if available; otherwise keep Markdown and HTML in sync manually.

**Step 4: Commit**

```bash
git add docs/integrations/database-source-integration.zh-CN.md docs/integrations/database-source-integration.zh-CN.html apps/web/app/lib/external-integrations.js apps/web/public/external-integrations/third-party-integration-api.zh-CN.md apps/web/public/external-integrations/third-party-integration-api.zh-CN.html
git commit -m "Document database source integration"
```

---

### Task 9: Deploy and Smoke Test on 8 Server

**Files:**
- Server env only: `/etc/aiv3/aiv3.env` or dedicated secret file
- No repository commit for secrets

**Step 1: Add server-side secret**

Add only on 8 server:

```env
THIRD_PARTY_HY_SQL_DATABASE_URL=mysql://<user>:<password>@8.155.12.154:23306/hy_sql?charset=utf8mb4
```

Do not write this into git.

**Step 2: Build release binaries**

Run on 8 server:

```bash
cd /srv/aiv3/repo
git pull --ff-only
npm --prefix apps/web run build
CC=clang CXX=clang++ cargo build --release -p platform-api -p external-source-worker
systemctl restart aiv3-platform-api.service aiv3-external-source-worker.service aiv3-web.service
systemctl is-active aiv3-platform-api.service aiv3-external-source-worker.service aiv3-web.service
```

Expected: all services active.

**Step 3: Smoke test schema inspection**

Use the main system UI or API to test:
- connection succeeds;
- schema shows database `hy_sql`;
- current test DB shows table `s`;
- row count is `0`;
- no password appears in response.

**Step 4: Smoke test empty sync**

Run a sync with table `s` mapped to a document using column `a` as content.

Expected:
- sync accepted;
- worker completes;
- document count is `0`;
- target dataset remains valid and empty;
- no ingest/index errors;
- no source DB writes.

**Step 5: Smoke test after real DDL lands**

After third party imports real test tables:
- inspect schema;
- configure mapping;
- preview first 5 rows;
- run full sync into a test V3 dataset;
- confirm documents parse and become `indexed`;
- test chat over selected dataset.
- generate a simple report/table from the selected dataset to confirm the report path sees database-derived evidence.

---

## Rollout Recommendation

1. Ship shared connector config and schema inspector first.
2. Add UI test/inspect without enabling sync.
3. Add mapping preview with row limit.
4. Enable full sync on copied test database only.
5. Enable incremental sync after `updated_at` or stable id/version strategy is confirmed.
6. Only then point at production-like read-only database.

## Final Acceptance Criteria

- V3 can store a MySQL source connection without storing raw credentials.
- Main system data source page can test connection and inspect schema.
- Main system data source page can bind/select the target V3 dataset before sync.
- Unsafe raw secrets and arbitrary SQL are rejected.
- Operators can map one or more MySQL tables to V3 external documents.
- Full sync can create V3 external documents and pass them into existing parse/index workflow.
- Incremental sync can use `updated_at`, numeric id, or version columns.
- Database-derived content can answer questions and generate reports only through selected/visible datasets after indexing.
- MySQL sync fails clearly when no effective target dataset is supplied or configured.
- Sync status shows row/document counts, failures, checkpoint, and drift warnings.
- Existing HTTP third-party source sync remains unchanged.
- Current chat, document parse, model pool, and external integration endpoints are unaffected unless a database sync is explicitly started.
