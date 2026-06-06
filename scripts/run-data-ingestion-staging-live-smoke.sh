#!/usr/bin/env bash
set -euo pipefail

repo_root="${DATA_INGESTION_LIVE_SMOKE_REPO_ROOT:-}"
if [[ -z "${repo_root}" ]]; then
  script_source="${BASH_SOURCE[0]:-$0}"
  script_dir="$(cd "$(dirname "${script_source}")" && pwd)"
  repo_root="$(cd "${script_dir}/.." && pwd)"
fi
cd "${repo_root}"

source_key="${DATA_INGESTION_LIVE_SMOKE_SOURCE_KEY:-hy-sql-traffic-area}"
report_dir="${DATA_INGESTION_LIVE_SMOKE_REPORT_DIR:-${repo_root}/target/data-ingestion-staging-live-smoke}"
report_basename="data-ingestion-staging-live-smoke-${source_key}-$(date -u +%Y%m%dT%H%M%SZ)"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
api_base="${DATA_INGESTION_LIVE_SMOKE_API_BASE:-http://127.0.0.1:3000}"
api_timeout="${DATA_INGESTION_LIVE_SMOKE_API_TIMEOUT_SECONDS:-20}"
require_default_ready="${DATA_INGESTION_LIVE_SMOKE_REQUIRE_DEFAULT_READY:-false}"
self_test="${DATA_INGESTION_LIVE_SMOKE_SELF_TEST:-false}"
psql_bin="${PSQL_BIN:-psql}"

if [[ "${self_test}" != "true" ]] && ! command -v "${psql_bin}" >/dev/null 2>&1; then
  echo "psql was not found. Install PostgreSQL client tools or set PSQL_BIN=/path/to/psql." >&2
  exit 1
fi

mkdir -p "${report_dir}"

started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
head_short="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

psql_args=(-X -v ON_ERROR_STOP=1 -At)
if [[ -n "${DATA_INGESTION_LIVE_SMOKE_DATABASE_URL:-}" ]]; then
  psql_args+=("${DATA_INGESTION_LIVE_SMOKE_DATABASE_URL}")
else
  psql_args+=(
    -h "${DATA_INGESTION_LIVE_SMOKE_DB_HOST:-127.0.0.1}"
    -p "${DATA_INGESTION_LIVE_SMOKE_DB_PORT:-5432}"
    -U "${DATA_INGESTION_LIVE_SMOKE_DB_USER:-ai_platform_v3}"
    -d "${DATA_INGESTION_LIVE_SMOKE_DB_NAME:-ai_data_platform_v3}"
  )
fi
psql_args+=(-v "source_key=${source_key}")

run_psql_json() {
  "${psql_bin}" "${psql_args[@]}" "$@"
}

echo "Data-ingestion staging live smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Source key: ${source_key}"
echo "Report directory: ${report_dir}"
echo "Postgres target: server-provided configuration (credentials are not printed)"
echo "API status endpoint: ${api_base}/v1/external/sources/${source_key}/database/status"

if [[ "${self_test}" == "true" ]]; then
  echo "Self-test mode: using synthetic V3 source/sync/dataset fixtures"
  source_json='{"source_id":"source-smoke","source_key":"hy-sql-traffic-area","connector_kind":"mysql","status":"enabled","health_status":"healthy","display_name":"HY SQL Traffic Area","database":"hy_sql","connection_env_present":true,"default_dataset_id":"dataset-smoke","mapped_table_count":1,"last_sync_at":"2026-05-30T09:00:00Z","last_success_at":"2026-05-30T09:00:00Z","last_failure_at":null,"updated_at":"2026-05-30T09:05:00Z"}'
  sync_runs_json='[{"sync_run_id":"sync-smoke","sync_kind":"content","status":"succeeded","failure_kind":null,"workflow_stage":"completed","workflow_status":"succeeded","documents_ingested":50,"chunks_ingested":50,"chunks_indexed":50,"retrieval_evidences_indexed":50,"enqueued_task_count":0,"created_at":"2026-05-30T08:55:00Z","updated_at":"2026-05-30T09:00:00Z"}]'
  datasets_json='[{"dataset_id":"dataset-smoke","key":"external-source-hy-sql-auto-dataset-hy-sql-main","title":"HY SQL Ready Dataset","lifecycle":"active","is_default":true,"dataset_external_id":"hy-sql-main","document_count":50,"indexed_document_count":50,"failed_document_count":0,"processing_document_count":0,"chunk_count":50,"indexed_chunk_count":50,"retrieval_evidence_count":50,"latest_document_updated_at":"2026-05-30T09:00:00Z","updated_at":"2026-05-30T09:05:00Z"}]'
  tables_json='[{"table":"bi_traffic_area","document_count":50,"indexed_document_count":50,"chunk_count":50,"indexed_chunk_count":50,"latest_document_updated_at":"2026-05-30T09:00:00Z"}]'
  identity_audit_json='{"source_id":"source-smoke","source_key":"hy-sql-traffic-area","latest_sync":{"sync_run_id":"sync-smoke","status":"succeeded","updated_at":"2026-05-30T09:00:00Z"},"tables":[{"table":"bi_traffic_area","id_column":"id","id_columns":["id"],"source_row_count":50,"unique_document_count":50,"unique_chunk_count":50,"collapsed_duplicate_row_count":0,"current_document_count":50}]}'
  api_status_json='{"source_id":"source-smoke","status":{"config_valid":true,"dataset_readiness":{"signal":"ready"},"sync_readiness":{"signal":"ready"},"health_findings":{"signal":"healthy","items":[]}}}'
  api_fetch_status="self_test"
else
source_json="$(
  run_psql_json <<'SQL'
select coalesce(
  (
    select jsonb_build_object(
      'source_id', id,
      'source_key', source_key,
      'connector_kind', connector_kind,
      'status', status,
      'health_status', health_status,
      'display_name', display_name,
      'database', nullif(config_redacted #>> '{database_source,database}', ''),
      'connection_env_present', nullif(config_redacted #>> '{database_source,connection_env}', '') is not null,
      'default_dataset_id', nullif(config_redacted #>> '{database_source,default_dataset_id}', ''),
      'mapped_table_count', coalesce(jsonb_array_length(coalesce(config_redacted #> '{database_source,tables}', '[]'::jsonb)), 0),
      'last_sync_at', last_sync_at,
      'last_success_at', last_success_at,
      'last_failure_at', last_failure_at,
      'updated_at', updated_at
    )
    from external_source_connections
    where source_key = :'source_key'
    order by updated_at desc
    limit 1
  ),
  'null'::jsonb
)::text;
SQL
)"

sync_runs_json="$(
  run_psql_json <<'SQL'
with source as (
  select id, tenant_id
  from external_source_connections
  where source_key = :'source_key'
  order by updated_at desc
  limit 1
),
runs as (
  select jsonb_build_object(
    'sync_run_id', r.id,
    'sync_kind', r.sync_kind,
    'status', r.status,
    'failure_kind', r.failure_kind,
    'workflow_stage', nullif(r.checkpoint #>> '{workflow_stage}', ''),
    'workflow_status', nullif(r.checkpoint #>> '{workflow_status}', ''),
    'documents_ingested', coalesce((r.counts ->> 'documents_ingested')::bigint, (r.counts ->> 'documents_created')::bigint, 0),
    'chunks_ingested', coalesce((r.counts ->> 'chunks_ingested')::bigint, 0),
    'chunks_indexed', coalesce((r.counts ->> 'chunks_indexed')::bigint, 0),
    'retrieval_evidences_indexed', coalesce((r.counts ->> 'retrieval_evidences_indexed')::bigint, 0),
    'enqueued_task_count', coalesce((r.counts ->> 'enqueued_task_count')::bigint, 0),
    'created_at', r.created_at,
    'updated_at', r.updated_at
  ) as item
  from external_sync_runs r
  join source s on s.id = r.source_id and s.tenant_id = r.tenant_id
  order by r.updated_at desc
  limit 10
)
select coalesce(jsonb_agg(item), '[]'::jsonb)::text
from runs;
SQL
)"

datasets_json="$(
  run_psql_json <<'SQL'
with source as (
  select id, tenant_id, nullif(config_redacted #>> '{database_source,default_dataset_id}', '')::uuid as default_dataset_id
  from external_source_connections
  where source_key = :'source_key'
  order by updated_at desc
  limit 1
),
source_dataset_ids as (
  select default_dataset_id as dataset_id, true as is_default
  from source
  where default_dataset_id is not null
  union
  select d.id, false
  from datasets d
  join source s on s.tenant_id = d.tenant_id
  where d.lifecycle <> 'archived'
    and d.metadata #>> '{external_source,source_id}' = s.id
  union
  select doc.dataset_id, false
  from documents doc
  join source s on s.tenant_id = doc.tenant_id
  where doc.lifecycle <> 'archived'
    and doc.metadata #>> '{external_source,source_id}' = s.id
),
summaries as (
  select d.id,
         d.key,
         d.title,
         d.lifecycle,
         bool_or(coalesce(sdi.is_default, false)) as is_default,
         nullif(d.metadata #>> '{external_source,dataset_external_id}', '') as dataset_external_id,
         count(distinct doc.id)::bigint as document_count,
         count(distinct doc.id) filter (where doc.lifecycle = 'indexed')::bigint as indexed_document_count,
         count(distinct doc.id) filter (where doc.lifecycle = 'failed')::bigint as failed_document_count,
         count(distinct doc.id) filter (where doc.lifecycle in ('received', 'extracted'))::bigint as processing_document_count,
         count(distinct ch.id)::bigint as chunk_count,
         count(distinct ch.id) filter (where ch.state = 'indexed')::bigint as indexed_chunk_count,
         count(distinct ev.id)::bigint as retrieval_evidence_count,
         max(doc.updated_at) as latest_document_updated_at,
         d.updated_at
  from source_dataset_ids sdi
  join source src on true
  join datasets d on d.tenant_id = src.tenant_id and d.id = sdi.dataset_id and d.lifecycle <> 'archived'
  left join documents doc
    on doc.tenant_id = src.tenant_id
   and doc.dataset_id = d.id
   and doc.lifecycle <> 'archived'
   and doc.metadata #>> '{external_source,source_id}' = src.id
  left join document_chunks ch
    on ch.tenant_id = src.tenant_id
   and ch.document_id = doc.id
  left join retrieval_evidences ev
    on ev.tenant_id = src.tenant_id
   and ev.document_id = doc.id
  group by d.id, d.key, d.title, d.lifecycle, d.metadata, d.updated_at
)
select coalesce(
  jsonb_agg(
    jsonb_build_object(
      'dataset_id', id,
      'key', key,
      'title', title,
      'lifecycle', lifecycle,
      'is_default', is_default,
      'dataset_external_id', dataset_external_id,
      'document_count', document_count,
      'indexed_document_count', indexed_document_count,
      'failed_document_count', failed_document_count,
      'processing_document_count', processing_document_count,
      'chunk_count', chunk_count,
      'indexed_chunk_count', indexed_chunk_count,
      'retrieval_evidence_count', retrieval_evidence_count,
      'latest_document_updated_at', latest_document_updated_at,
      'updated_at', updated_at
    )
    order by is_default desc, indexed_document_count desc, updated_at desc
  ),
  '[]'::jsonb
)::text
from summaries;
SQL
)"

tables_json="$(
  run_psql_json <<'SQL'
with source as (
  select id, tenant_id
  from external_source_connections
  where source_key = :'source_key'
  order by updated_at desc
  limit 1
),
docs as (
  select doc.id,
         doc.lifecycle,
         doc.updated_at,
         coalesce(nullif(doc.metadata #>> '{external_metadata,source_table}', ''), '[unmapped]') as source_table
  from documents doc
  join source s on s.tenant_id = doc.tenant_id
  where doc.lifecycle <> 'archived'
    and doc.metadata #>> '{external_source,source_id}' = s.id
),
summaries as (
  select source_table,
         count(distinct id)::bigint as document_count,
         count(distinct id) filter (where lifecycle = 'indexed')::bigint as indexed_document_count,
         max(updated_at) as latest_document_updated_at
  from docs
  group by source_table
),
chunk_summaries as (
  select d.source_table,
         count(c.id)::bigint as chunk_count,
         count(c.id) filter (where c.state = 'indexed')::bigint as indexed_chunk_count
  from docs d
  left join document_chunks c on c.document_id = d.id
  group by d.source_table
)
select coalesce(
  jsonb_agg(
    jsonb_build_object(
      'table', s.source_table,
      'document_count', s.document_count,
      'indexed_document_count', s.indexed_document_count,
      'chunk_count', coalesce(c.chunk_count, 0),
      'indexed_chunk_count', coalesce(c.indexed_chunk_count, 0),
      'latest_document_updated_at', s.latest_document_updated_at
    )
    order by s.indexed_document_count desc, s.source_table asc
  ),
  '[]'::jsonb
)::text
from summaries s
left join chunk_summaries c on c.source_table = s.source_table;
SQL
)"

identity_audit_json="$(
  run_psql_json <<'SQL'
with source as (
  select id, tenant_id, config_redacted
  from external_source_connections
  where source_key = :'source_key'
  order by updated_at desc
  limit 1
),
mapped_tables as (
  select coalesce(
           nullif(item ->> 'table', ''),
           nullif(item ->> 'name', ''),
           nullif(item ->> 'table_name', '')
         ) as table_name,
         nullif(item ->> 'id_column', '') as id_column,
         case
           when jsonb_typeof(item -> 'id_columns') = 'array'
                and jsonb_array_length(item -> 'id_columns') > 0
             then item -> 'id_columns'
           when nullif(item ->> 'id_column', '') is not null
             then jsonb_build_array(item ->> 'id_column')
           else '[]'::jsonb
         end as id_columns
  from source s
  cross join lateral jsonb_array_elements(
    coalesce(s.config_redacted #> '{database_source,tables}', '[]'::jsonb)
  ) as mapped(item)
),
latest_sync as (
  select r.id as sync_run_id,
         r.status,
         r.updated_at,
         r.counts
  from external_sync_runs r
  join source s on s.id = r.source_id and s.tenant_id = r.tenant_id
  where jsonb_typeof(coalesce(r.counts -> 'ingest_table_counts', 'null'::jsonb)) = 'array'
  order by r.updated_at desc
  limit 1
),
sync_counts as (
  select nullif(item ->> 'table', '') as table_name,
         coalesce(nullif(item ->> 'row_count', '')::bigint, nullif(item ->> 'source_rows_ingested', '')::bigint, 0) as source_row_count,
         coalesce(nullif(item ->> 'unique_documents_materialized', '')::bigint, nullif(item ->> 'documents_ingested', '')::bigint, 0) as unique_document_count,
         coalesce(nullif(item ->> 'unique_chunks_materialized', '')::bigint, nullif(item ->> 'chunks_ingested', '')::bigint, 0) as unique_chunk_count,
         coalesce(nullif(item ->> 'collapsed_duplicate_row_count', '')::bigint, 0) as collapsed_duplicate_row_count
  from latest_sync
  cross join lateral jsonb_array_elements(latest_sync.counts -> 'ingest_table_counts') as counts(item)
),
doc_counts as (
  select coalesce(nullif(doc.metadata #>> '{external_metadata,source_table}', ''), '[unmapped]') as table_name,
         count(distinct doc.id)::bigint as current_document_count
  from source s
  join documents doc on doc.tenant_id = s.tenant_id
  where doc.lifecycle <> 'archived'
    and doc.metadata #>> '{external_source,source_id}' = s.id
  group by coalesce(nullif(doc.metadata #>> '{external_metadata,source_table}', ''), '[unmapped]')
),
table_names as (
  select table_name from mapped_tables where table_name is not null
  union
  select table_name from sync_counts where table_name is not null
  union
  select table_name from doc_counts where table_name is not null
),
table_rows as (
  select jsonb_build_object(
           'table', n.table_name,
           'id_column', m.id_column,
           'id_columns', coalesce(m.id_columns, '[]'::jsonb),
           'source_row_count', coalesce(sc.source_row_count, 0),
           'unique_document_count', coalesce(sc.unique_document_count, 0),
           'unique_chunk_count', coalesce(sc.unique_chunk_count, 0),
           'collapsed_duplicate_row_count',
             case
               when sc.table_name is null then 0
               when sc.collapsed_duplicate_row_count > 0 then sc.collapsed_duplicate_row_count
               else greatest(sc.source_row_count - sc.unique_document_count, 0)
             end,
           'current_document_count', coalesce(dc.current_document_count, 0)
         ) as item
  from table_names n
  left join mapped_tables m on m.table_name = n.table_name
  left join sync_counts sc on sc.table_name = n.table_name
  left join doc_counts dc on dc.table_name = n.table_name
)
select jsonb_build_object(
  'source_id', (select id from source),
  'source_key', :'source_key',
  'latest_sync', coalesce(
    (
      select jsonb_build_object(
        'sync_run_id', sync_run_id,
        'status', status,
        'updated_at', updated_at
      )
      from latest_sync
    ),
    'null'::jsonb
  ),
  'tables', coalesce(
    (
      select jsonb_agg(item order by item ->> 'table')
      from table_rows
    ),
    '[]'::jsonb
  )
)::text;
SQL
)"

api_status_json="null"
api_fetch_status="skipped"
if command -v curl >/dev/null 2>&1; then
  if api_status_json="$(curl -sS -m "${api_timeout}" "${api_base}/v1/external/sources/${source_key}/database/status" 2>/dev/null)"; then
    api_fetch_status="fetched"
  else
    api_status_json="null"
    api_fetch_status="failed"
  fi
fi
fi

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

SMOKE_SOURCE_KEY="${source_key}" \
SMOKE_HEAD="${head_short}" \
SMOKE_STARTED_AT="${started_at}" \
SMOKE_FINISHED_AT="${finished_at}" \
SMOKE_REQUIRE_DEFAULT_READY="${require_default_ready}" \
SMOKE_API_FETCH_STATUS="${api_fetch_status}" \
SMOKE_SOURCE_JSON="${source_json}" \
SMOKE_SYNC_RUNS_JSON="${sync_runs_json}" \
SMOKE_DATASETS_JSON="${datasets_json}" \
SMOKE_TABLES_JSON="${tables_json}" \
SMOKE_IDENTITY_AUDIT_JSON="${identity_audit_json}" \
SMOKE_API_STATUS_JSON="${api_status_json}" \
node >"${report_json}" <<'NODE'
const parseJson = (value, fallback) => {
  try {
    if (!value || !String(value).trim()) return fallback;
    return JSON.parse(value);
  } catch {
    return fallback;
  }
};

const source = parseJson(process.env.SMOKE_SOURCE_JSON, null);
const syncRuns = parseJson(process.env.SMOKE_SYNC_RUNS_JSON, []);
const datasets = parseJson(process.env.SMOKE_DATASETS_JSON, []);
const tables = parseJson(process.env.SMOKE_TABLES_JSON, []);
const identityAuditRaw = parseJson(process.env.SMOKE_IDENTITY_AUDIT_JSON, null);
const apiStatus = parseJson(process.env.SMOKE_API_STATUS_JSON, null);
const requireDefaultReady = process.env.SMOKE_REQUIRE_DEFAULT_READY === 'true';
const sourceExists = Boolean(source && source.source_id);
const datasetReady = (dataset) =>
  Number(dataset?.indexed_document_count || 0) > 0 &&
  Number(dataset?.indexed_chunk_count || 0) > 0;
const readyDatasets = datasets.filter(datasetReady);
const defaultDataset = datasets.find((dataset) => dataset.is_default) || null;
const defaultDatasetReady = defaultDataset ? datasetReady(defaultDataset) : false;
const hasSucceededSync = syncRuns.some((run) => ['succeeded', 'completed'].includes(String(run.status || '').toLowerCase()));
const latestSync = syncRuns[0] || null;
const latestSyncFailed = latestSync
  ? ['failed', 'cancelled', 'dead_lettered'].includes(String(latestSync.status || '').toLowerCase()) ||
    Boolean(latestSync.failure_kind)
  : false;
const sortedReadyDatasets = [...readyDatasets].sort((left, right) => {
  const leftDefault = left.is_default ? 1 : 0;
  const rightDefault = right.is_default ? 1 : 0;
  if (leftDefault !== rightDefault) return rightDefault - leftDefault;
  return Number(right.retrieval_evidence_count || 0) - Number(left.retrieval_evidence_count || 0);
});
const primaryReadyDataset = sortedReadyDatasets[0] || null;
const readyTables = tables.filter((table) =>
  Number(table?.indexed_document_count || 0) > 0 &&
  Number(table?.indexed_chunk_count || 0) > 0
);
const apiSummary = apiStatus && typeof apiStatus === 'object'
  ? {
      source_id: apiStatus.source_id || null,
      config_valid: apiStatus.status?.config_valid ?? null,
      dataset_signal: apiStatus.status?.dataset_readiness?.signal || null,
      sync_signal: apiStatus.status?.sync_readiness?.signal || null,
      health_signal: apiStatus.status?.health_findings?.signal || null,
      health_item_codes: Array.isArray(apiStatus.status?.health_findings?.items)
        ? apiStatus.status.health_findings.items.map((item) => item.code).filter(Boolean)
        : [],
    }
  : null;
const normalizeIdentityAudit = (audit) => {
  const empty = {
    source_id: null,
    source_key: process.env.SMOKE_SOURCE_KEY,
    latest_sync: null,
    tables: [],
    collapsed_table_count: 0,
    total_collapsed_duplicate_row_count: 0,
  };
  if (!audit || typeof audit !== 'object') return empty;
  const tables = Array.isArray(audit.tables)
    ? audit.tables.map((table) => {
        const idColumns = Array.isArray(table.id_columns)
          ? table.id_columns.filter((column) => typeof column === 'string' && column.trim()).map((column) => column.trim())
          : [];
        const sourceRowCount = Number(table.source_row_count || 0);
        const uniqueDocumentCount = Number(table.unique_document_count || 0);
        const collapsedDuplicateRowCount = Number(
          table.collapsed_duplicate_row_count ?? Math.max(sourceRowCount - uniqueDocumentCount, 0)
        );
        const identityStatus =
          sourceRowCount <= 0
            ? 'no_latest_sync_rows'
            : uniqueDocumentCount <= 0
              ? 'not_materialized'
              : collapsedDuplicateRowCount > 0
                ? 'collapsed_identity'
                : 'row_level_or_no_duplicates';
        const recommendedAction =
          identityStatus !== 'collapsed_identity'
            ? 'none'
            : idColumns.length > 1
              ? 'verify composite identity is active in a staging sync before relying on row-level reports'
              : 'consider a composite identity mapping in staging if source-row-level completeness is required';
        return {
          table: table.table || '[unmapped]',
          id_column: table.id_column || null,
          id_columns: idColumns,
          source_row_count: sourceRowCount,
          unique_document_count: uniqueDocumentCount,
          unique_chunk_count: Number(table.unique_chunk_count || 0),
          collapsed_duplicate_row_count: collapsedDuplicateRowCount,
          current_document_count: Number(table.current_document_count || 0),
          identity_status: identityStatus,
          recommended_action: recommendedAction,
        };
      })
    : [];
  return {
    source_id: audit.source_id || null,
    source_key: audit.source_key || process.env.SMOKE_SOURCE_KEY,
    latest_sync: audit.latest_sync || null,
    tables,
    collapsed_table_count: tables.filter((table) => table.identity_status === 'collapsed_identity').length,
    total_collapsed_duplicate_row_count: tables.reduce(
      (sum, table) => sum + Number(table.collapsed_duplicate_row_count || 0),
      0
    ),
  };
};
const identityAudit = normalizeIdentityAudit(identityAuditRaw);
const warnings = [];
if (sourceExists && !source.connection_env_present) {
  warnings.push('database source has no configured connection env reference');
}
if (sourceExists && source.status !== 'enabled') {
  warnings.push(`database source status is ${source.status}`);
}
if (!defaultDataset) {
  warnings.push('database source has no visible default dataset');
} else if (!defaultDatasetReady) {
  warnings.push('default dataset has no indexed database-source documents yet');
}
if (latestSyncFailed) {
  warnings.push(`latest sync is ${latestSync.status}${latestSync.failure_kind ? ` (${latestSync.failure_kind})` : ''}`);
}
if (!hasSucceededSync) {
  warnings.push('no succeeded sync run was found for this source');
}
if (!readyDatasets.length) {
  warnings.push('no source-derived dataset has indexed documents and chunks');
}
if (identityAudit.collapsed_table_count > 0) {
  warnings.push(
    `identity audit found collapsed source rows in ${identityAudit.collapsed_table_count} table(s)`
  );
}
const ready = sourceExists && hasSucceededSync && readyDatasets.length > 0 && (!requireDefaultReady || defaultDatasetReady);
const questionReportReady = Boolean(
  ready &&
    primaryReadyDataset &&
    Number(primaryReadyDataset.retrieval_evidence_count || 0) > 0 &&
    readyTables.length > 0
);
const shellSingleQuote = (value) => `'${String(value).replace(/'/g, `'\\''`)}'`;
const reportCommand = `HY_SQL_TRAFFIC_SOURCE_ID=${shellSingleQuote(process.env.SMOKE_SOURCE_KEY || 'hy-sql-traffic-area')} bash scripts/run-hy-sql-traffic-area-live-report.sh`;

const report = {
  smoke: 'data-ingestion-staging-live',
  ready,
  source_key: process.env.SMOKE_SOURCE_KEY,
  head: process.env.SMOKE_HEAD,
  started_at: process.env.SMOKE_STARTED_AT,
  finished_at: process.env.SMOKE_FINISHED_AT,
  safety_contract: {
    reads_v3_postgres_only: true,
    source_database_read: false,
    writes_allowed: false,
    raw_credentials_printed: false,
    raw_table_dump_allowed: false,
  },
  checks: {
    source_exists: sourceExists,
    source_enabled: source?.status === 'enabled',
    connection_env_present: Boolean(source?.connection_env_present),
    mapped_table_count: Number(source?.mapped_table_count || 0),
    has_succeeded_sync: hasSucceededSync,
    latest_sync_failed: latestSyncFailed,
    dataset_count: datasets.length,
    ready_dataset_count: readyDatasets.length,
    default_dataset_ready: defaultDatasetReady,
    require_default_ready: requireDefaultReady,
    question_report_ready: questionReportReady,
  },
  source: source
    ? {
        source_id: source.source_id,
        connector_kind: source.connector_kind,
        status: source.status,
        health_status: source.health_status,
        display_name: source.display_name,
        database: source.database,
        connection_env_present: Boolean(source.connection_env_present),
        default_dataset_id: source.default_dataset_id || null,
        mapped_table_count: Number(source.mapped_table_count || 0),
        last_sync_at: source.last_sync_at || null,
        last_success_at: source.last_success_at || null,
        last_failure_at: source.last_failure_at || null,
      }
    : null,
  datasets,
  ready_datasets: readyDatasets.map((dataset) => ({
    dataset_id: dataset.dataset_id,
    key: dataset.key,
    title: dataset.title,
    is_default: Boolean(dataset.is_default),
    indexed_document_count: Number(dataset.indexed_document_count || 0),
    indexed_chunk_count: Number(dataset.indexed_chunk_count || 0),
    retrieval_evidence_count: Number(dataset.retrieval_evidence_count || 0),
  })),
  table_readiness: tables,
  latest_sync_identity_audit: identityAudit,
  question_report_readiness: {
    ready: questionReportReady,
    basis: primaryReadyDataset
      ? {
          dataset_id: primaryReadyDataset.dataset_id,
          dataset_key: primaryReadyDataset.key,
          dataset_title: primaryReadyDataset.title,
          is_default: Boolean(primaryReadyDataset.is_default),
          indexed_document_count: Number(primaryReadyDataset.indexed_document_count || 0),
          indexed_chunk_count: Number(primaryReadyDataset.indexed_chunk_count || 0),
          retrieval_evidence_count: Number(primaryReadyDataset.retrieval_evidence_count || 0),
        }
      : null,
    ready_table_count: readyTables.length,
    ready_tables: readyTables.slice(0, 8).map((table) => ({
      table: table.table,
      indexed_document_count: Number(table.indexed_document_count || 0),
      indexed_chunk_count: Number(table.indexed_chunk_count || 0),
    })),
    can_answer_dataset_questions: questionReportReady,
    can_generate_static_page_report: questionReportReady,
    suggested_questions: [
      '这个数据库主要记录什么业务内容？',
      '按主要维度做一个排行表，列出 Top 10 和口径说明。',
      '基于当前数据生成一份经营分析静态页报表。',
    ],
    suggested_report_command: reportCommand,
  },
  recent_sync_runs: syncRuns,
  api_status: {
    fetch_status: process.env.SMOKE_API_FETCH_STATUS,
    summary: apiSummary,
  },
  warnings,
  notes: [
    'This live smoke validates V3-stored source, sync, dataset, chunk, and evidence state only.',
    'It does not connect to or query the customer/source database.',
    'A latest failed sync can coexist with an older ready dataset; the report separates those signals.',
    'Question/report readiness means V3 has a source-derived dataset with indexed documents, chunks, retrieval evidence, and at least one ready source table.',
    'The identity audit is read-only and compares the latest DataMax sync counts with configured identity columns; collapsed rows are an attention signal, not an automatic smoke failure.',
  ],
};

process.stdout.write(JSON.stringify(report, null, 2));
NODE

SMOKE_REPORT_JSON="${report_json}" node >"${report_md}" <<'NODE'
const fs = require('fs');
const report = JSON.parse(fs.readFileSync(process.env.SMOKE_REPORT_JSON, 'utf8'));
const bool = (value) => (value ? 'yes' : 'no');
const lines = [
  '# Data Ingestion Staging Live Smoke',
  '',
  `- Status: ${report.ready ? 'passed' : 'attention_required'}`,
  `- Source key: \`${report.source_key}\``,
  `- HEAD: ${report.head}`,
  `- Started: ${report.started_at}`,
  `- Finished: ${report.finished_at}`,
  '',
  '## Checks',
  '',
  `- Source exists: ${bool(report.checks.source_exists)}`,
  `- Source enabled: ${bool(report.checks.source_enabled)}`,
  `- Connection env reference present: ${bool(report.checks.connection_env_present)}`,
  `- Mapped table count: ${report.checks.mapped_table_count}`,
  `- Succeeded sync exists: ${bool(report.checks.has_succeeded_sync)}`,
  `- Latest sync failed: ${bool(report.checks.latest_sync_failed)}`,
  `- Dataset count: ${report.checks.dataset_count}`,
  `- Ready dataset count: ${report.checks.ready_dataset_count}`,
  `- Default dataset ready: ${bool(report.checks.default_dataset_ready)}`,
  `- Require default ready: ${bool(report.checks.require_default_ready)}`,
  `- Question/report ready: ${bool(report.checks.question_report_ready)}`,
  '',
  '## Ready Datasets',
  '',
  ...(report.ready_datasets.length
    ? report.ready_datasets.map((dataset) =>
        `- \`${dataset.key}\`: docs=${dataset.indexed_document_count}, chunks=${dataset.indexed_chunk_count}, evidence=${dataset.retrieval_evidence_count}, default=${bool(dataset.is_default)}`
      )
    : ['- none']),
  '',
  '## Recent Sync Runs',
  '',
  ...(report.recent_sync_runs.length
    ? report.recent_sync_runs.slice(0, 5).map((run) =>
        `- ${run.status}: \`${run.sync_kind}\`, docs=${run.documents_ingested}, chunks=${run.chunks_ingested}, evidence=${run.retrieval_evidences_indexed}, failure=${run.failure_kind || 'none'}`
      )
    : ['- none']),
  '',
  '## API Status Snapshot',
  '',
  `- Fetch: ${report.api_status.fetch_status}`,
  `- Dataset signal: ${report.api_status.summary?.dataset_signal || 'n/a'}`,
  `- Sync signal: ${report.api_status.summary?.sync_signal || 'n/a'}`,
  `- Health signal: ${report.api_status.summary?.health_signal || 'n/a'}`,
  `- Health codes: ${(report.api_status.summary?.health_item_codes || []).join(', ') || 'none'}`,
  '',
  '## Latest Sync Identity Audit',
  '',
  `- Latest sync: ${report.latest_sync_identity_audit.latest_sync?.sync_run_id || 'none'}`,
  `- Collapsed table count: ${report.latest_sync_identity_audit.collapsed_table_count}`,
  `- Collapsed duplicate rows: ${report.latest_sync_identity_audit.total_collapsed_duplicate_row_count}`,
  '',
  ...(report.latest_sync_identity_audit.tables.length
    ? [
        '| Table | Identity Columns | Source Rows | Unique Docs | Collapsed Rows | Current Docs | Status | Recommended Action |',
        '| --- | --- | ---: | ---: | ---: | ---: | --- | --- |',
        ...report.latest_sync_identity_audit.tables.map((table) =>
          `| \`${table.table}\` | ${table.id_columns.length ? table.id_columns.map((column) => `\`${column}\``).join(', ') : 'none'} | ${table.source_row_count} | ${table.unique_document_count} | ${table.collapsed_duplicate_row_count} | ${table.current_document_count} | ${table.identity_status} | ${table.recommended_action || 'none'} |`
        ),
      ]
    : ['- none']),
  '',
  '## Question And Report Readiness',
  '',
  `- Ready: ${bool(report.question_report_readiness.ready)}`,
  `- Basis dataset: ${report.question_report_readiness.basis?.dataset_key || 'none'}`,
  `- Ready table count: ${report.question_report_readiness.ready_table_count}`,
  `- Can answer dataset questions: ${bool(report.question_report_readiness.can_answer_dataset_questions)}`,
  `- Can generate static-page report: ${bool(report.question_report_readiness.can_generate_static_page_report)}`,
  '- Suggested questions:',
  ...report.question_report_readiness.suggested_questions.map((question) => `  - ${question}`),
  `- Suggested report command: \`${report.question_report_readiness.suggested_report_command}\``,
  '',
  '## Warnings',
  '',
  ...(report.warnings.length ? report.warnings.map((warning) => `- ${warning}`) : ['- none']),
  '',
  '## Safety Contract',
  '',
  ...Object.entries(report.safety_contract).map(([key, value]) => `- ${key}: ${value}`),
  '',
].join('\n');

fs.writeFileSync(1, lines);
NODE

echo ""
echo "Data-ingestion staging live smoke report: ${report_json}"
echo "Data-ingestion staging live smoke summary: ${report_md}"
if node -e "const fs=require('fs'); const r=JSON.parse(fs.readFileSync(process.argv[1], 'utf8')); process.exit(r.ready ? 0 : 1)" "${report_json}"; then
  echo "OK data-ingestion-staging-live smoke completed."
else
  echo "WARN data-ingestion-staging-live smoke completed with attention_required. See report." >&2
  exit 2
fi
