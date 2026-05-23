import {
  databaseSourceSummary,
  normalizeDatabaseSourceStatus,
  normalizeIntegrationSummary,
  numberOrZero,
} from './external-integrations.js';

async function fetchJson(pathname, options = {}) {
  const headers = { accept: 'application/json', ...(options.headers || {}) };
  let body = options.body;
  if (body && typeof body === 'object' && !(body instanceof FormData)) {
    headers['content-type'] = 'application/json';
    body = JSON.stringify(body);
  }
  const response = await fetch(pathname, {
    method: options.method || 'GET',
    cache: 'no-store',
    headers,
    body,
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    const error = new Error(payload?.message || payload?.error || `请求失败 ${response.status}`);
    error.status = response.status;
    error.code = payload?.error || '';
    throw error;
  }
  return payload;
}

export function normalizeDatabaseSourceOption(integration = {}) {
  const source = databaseSourceSummary(integration);
  return {
    id: integration.id || integration.integration_id || '',
    displayName: integration.displayName || integration.display_name || integration.id || integration.integration_id || '未命名数据库源',
    provider: integration.provider || 'mysql',
    status: integration.status || 'unknown',
    healthStatus: integration.healthStatus || integration.health_status || 'unknown',
    lastSyncAt: integration.lastSyncAt || integration.last_sync_at || null,
    lastSuccessAt: integration.lastSuccessAt || integration.last_success_at || null,
    lastFailureAt: integration.lastFailureAt || integration.last_failure_at || null,
    source,
  };
}

export function databaseSourceOptionsFromIntegrations(payload = {}) {
  const integrations = Array.isArray(payload?.integrations)
    ? payload.integrations.map(normalizeIntegrationSummary)
    : Array.isArray(payload)
      ? payload.map(normalizeIntegrationSummary)
      : [];
  return integrations
    .map(normalizeDatabaseSourceOption)
    .filter((item) => item.id && item.source.configured);
}

export function normalizeDatabaseSchema(payload = {}) {
  const schema = payload?.schema && typeof payload.schema === 'object' ? payload.schema : payload;
  const tables = Array.isArray(schema?.tables)
    ? schema.tables.map((table) => ({
      name: String(table?.name || table?.table || ''),
      table: String(table?.table || table?.name || ''),
      columnCount: numberOrZero(table?.column_count ?? table?.columnCount ?? table?.columns?.length),
      approximateRowCount: numberOrZero(table?.approximate_row_count ?? table?.approximateRowCount ?? table?.row_count),
      columns: Array.isArray(table?.columns)
        ? table.columns.map((column) => ({
          name: String(column?.name || column?.column || ''),
          dataType: String(column?.data_type || column?.dataType || column?.type || ''),
          nullable: column?.nullable !== false,
        })).filter((column) => column.name)
        : [],
    })).filter((table) => table.table || table.name)
    : [];
  return {
    database: String(schema?.database || ''),
    tableCount: numberOrZero(schema?.table_count ?? schema?.tableCount ?? tables.length),
    tables,
  };
}

export function normalizeDatabaseProfile(payload = {}) {
  const profile = payload?.profile && typeof payload.profile === 'object' ? payload.profile : payload;
  const tables = Array.isArray(profile?.tables)
    ? profile.tables.map((table) => ({
      table: String(table?.table || table?.name || ''),
      approximateRowCount: numberOrZero(table?.approximate_row_count ?? table?.approximateRowCount),
      dimensionCount: numberOrZero(table?.dimension_count ?? table?.dimensionCount),
      metricCount: numberOrZero(table?.metric_count ?? table?.metricCount),
      timeDimensionCount: numberOrZero(table?.time_dimension_count ?? table?.timeDimensionCount),
      entityColumnCount: numberOrZero(table?.entity_column_count ?? table?.entityColumnCount),
      textColumnCount: numberOrZero(table?.text_column_count ?? table?.textColumnCount),
      mappingConfidence: numberOrZero(table?.mapping_confidence ?? table?.mappingConfidence),
    })).filter((table) => table.table)
    : [];
  return {
    database: String(profile?.database || ''),
    tableCount: numberOrZero(profile?.table_count ?? profile?.tableCount ?? tables.length),
    metricCount: numberOrZero(profile?.metric_count ?? profile?.metricCount),
    dimensionCount: numberOrZero(profile?.dimension_count ?? profile?.dimensionCount),
    tables,
  };
}

export async function fetchDatabaseSourceOptions() {
  return databaseSourceOptionsFromIntegrations(await fetchJson('/api/v3/external/integrations'));
}

export async function fetchDatabaseSourceStatus(sourceId) {
  const payload = await fetchJson(`/api/v3/external/sources/${encodeURIComponent(sourceId)}/database/status`);
  return normalizeDatabaseSourceStatus(payload);
}

export async function testDatabaseSourceConnection(sourceId) {
  return fetchJson(`/api/v3/external/sources/${encodeURIComponent(sourceId)}/database/test`, {
    method: 'POST',
    body: { database_source: {} },
  });
}

export async function inspectDatabaseSourceSchema(sourceId) {
  const payload = await fetchJson(`/api/v3/external/sources/${encodeURIComponent(sourceId)}/database/schema`, {
    method: 'POST',
    body: { database_source: {} },
  });
  return normalizeDatabaseSchema(payload);
}

export async function profileDatabaseSource(sourceId) {
  const payload = await fetchJson(`/api/v3/external/sources/${encodeURIComponent(sourceId)}/database/profile`, {
    method: 'POST',
    body: { sample_limit: 100, database_source: {} },
  });
  return normalizeDatabaseProfile(payload);
}

export function databaseSourceSyncRequestBody(syncKind = 'incremental', target = '') {
  const body = {
    sync_kind: syncKind,
    connector_context: {},
  };
  if (target && typeof target === 'object') {
    const datasetId = String(target.datasetId || target.dataset_id || '').trim();
    const datasetExternalId = String(target.datasetExternalId || target.dataset_external_id || '').trim();
    const datasetTitle = String(target.datasetTitle || target.dataset_title || '').trim();
    if (datasetId) {
      body.dataset_id = datasetId;
    } else if (datasetExternalId) {
      body.dataset_external_id = datasetExternalId;
      if (datasetTitle) {
        body.dataset_title = datasetTitle;
      }
    }
    return body;
  }
  const datasetId = String(target || '').trim();
  if (datasetId) {
    body.dataset_id = datasetId;
  }
  return body;
}

export async function startDatabaseSourceSync(sourceId, syncKind = 'incremental', target = '') {
  const body = databaseSourceSyncRequestBody(syncKind, target);
  return fetchJson(`/api/v3/external/sources/${encodeURIComponent(sourceId)}/sync`, {
    method: 'POST',
    body,
  });
}
