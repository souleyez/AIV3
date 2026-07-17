import { readLocalSecretBindingIdsHeader } from './local-account-state.js';
import { readLocalThreadId } from './local-browser-state.js';

function text(value) {
  return String(value ?? '').trim();
}

function requiredId(value, label) {
  const normalized = text(value);
  if (!normalized) throw new TypeError(`${label} is required`);
  return normalized;
}

function finiteInteger(value, fallback = 0) {
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, Math.floor(number)) : fallback;
}

function normalizeColumn(column = {}, index = 0) {
  const name = text(column.name);
  if (!name) return null;
  return {
    ordinal: finiteInteger(column.ordinal, index + 1) || index + 1,
    name,
  };
}

function normalizeTable(table = {}) {
  const documentId = text(table.document_id || table.documentId);
  const title = text(table.title);
  const columns = (Array.isArray(table.columns) ? table.columns : [])
    .map(normalizeColumn)
    .filter(Boolean)
    .sort((left, right) => left.ordinal - right.ordinal);
  if (!documentId || !title || !columns.length) return null;
  return {
    documentId,
    title,
    contentType: text(table.content_type || table.contentType),
    updatedAt: text(table.updated_at || table.updatedAt),
    structuralSource: text(table.structural_source || table.structuralSource) || 'file_header',
    columns,
  };
}

export function normalizeDatasetTabularSchema(payload = {}) {
  const tables = (Array.isArray(payload.tables) ? payload.tables : [])
    .map(normalizeTable)
    .filter(Boolean);
  return {
    schemaVersion: text(payload.schema_version || payload.schemaVersion),
    datasetId: text(payload.dataset_id || payload.datasetId),
    tabularDocumentCount: finiteInteger(
      payload.tabular_document_count ?? payload.tabularDocumentCount,
      tables.length,
    ),
    columnCount: finiteInteger(
      payload.column_count ?? payload.columnCount,
      tables.reduce((sum, table) => sum + table.columns.length, 0),
    ),
    skippedTabularDocumentCount: finiteInteger(
      payload.skipped_tabular_document_count ?? payload.skippedTabularDocumentCount,
    ),
    tables,
  };
}

async function responseError(response) {
  const contentType = response.headers.get('content-type') || '';
  const payload = contentType.includes('application/json')
    ? await response.json()
    : await response.text();
  const message = typeof payload === 'string'
    ? payload
    : payload?.message || payload?.error || `Request failed: ${response.status}`;
  const error = new Error(message);
  error.status = response.status;
  error.payload = payload;
  return error;
}

export function createDatasetTabularSchemaClient(dependencies = {}) {
  const {
    fetchImpl = (...args) => globalThis.fetch(...args),
    readSecretBindingIdsHeader = readLocalSecretBindingIdsHeader,
    readLocalThreadId: readThreadId = readLocalThreadId,
  } = dependencies;

  return async function fetchDatasetTabularSchema(datasetId, options = {}) {
    const id = requiredId(datasetId, 'datasetId');
    const secretBindingIds = readSecretBindingIdsHeader();
    const response = await fetchImpl(`/api/v3/datasets/${encodeURIComponent(id)}/tabular-schema`, {
      method: 'GET',
      cache: 'no-store',
      credentials: 'include',
      signal: options.signal,
      headers: {
        Accept: 'application/json',
        ...(secretBindingIds ? { 'X-AI-Data-Platform-Secret-Binding-Ids': secretBindingIds } : {}),
        'X-AI-Data-Platform-Local-Thread-Id': readThreadId(),
      },
    });
    if (!response.ok) throw await responseError(response);
    return normalizeDatasetTabularSchema(await response.json());
  };
}

export const fetchDatasetTabularSchema = createDatasetTabularSchemaClient();
