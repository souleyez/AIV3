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
psql_bin="${PSQL_BIN:-psql}"

if ! command -v "${psql_bin}" >/dev/null 2>&1; then
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
const ready = sourceExists && hasSucceededSync && readyDatasets.length > 0 && (!requireDefaultReady || defaultDatasetReady);

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
if node -e "const r=require(process.argv[1]); process.exit(r.ready ? 0 : 1)" "${report_json}"; then
  echo "OK data-ingestion-staging-live smoke completed."
else
  echo "WARN data-ingestion-staging-live smoke completed with attention_required. See report." >&2
  exit 2
fi
