import { isSensitiveSemanticExample } from './dataset-understanding-api.js';

const RECENT_WINDOW_MS = 24 * 60 * 60 * 1000;
const RECENT_DOCUMENT_LIMIT = 4;
const FIELD_HINT_LIMIT = 8;

function normalizedText(value) {
  return typeof value === 'string' || typeof value === 'number'
    ? String(value).trim()
    : '';
}

function uniqueTexts(values = [], limit = Number.POSITIVE_INFINITY) {
  const seen = new Set();
  const result = [];
  for (const value of values) {
    const text = normalizedText(value);
    if (!text) continue;
    const key = text.toLocaleLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(text);
    if (result.length >= limit) break;
  }
  return result;
}

function fieldHintText(value) {
  const text = normalizedText(value);
  if (!text || text.length > 64) return '';
  if (isSensitiveSemanticExample(text)) return '';
  if (/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(text)) return '';
  if (/^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)[A-Za-z0-9_-]{20,}$/.test(text)) return '';
  if (/^[a-f0-9]{24,}$/i.test(text)) return '';
  if (/[\r\n\t{}\[\]"]/.test(text)) return '';
  if (/[\\/]/.test(text) || /(?:https?:\/\/|www\.)/i.test(text)) return '';
  if (/\b[^\s@]+@[^\s@]+\.[^\s@]+\b/.test(text)) return '';
  if (/(?:token|secret|password|passwd|credential|api[_-]?key|access[_-]?key)\s*[:=]/i.test(text)) return '';
  if (/(?:bearer\s+|jdbc:|host\s*=|user(?:name)?\s*=)/i.test(text)) return '';
  if (text.includes('=') || text.includes('?') || text.includes(',')) return '';
  if (/^\d+(?:[.\-/:]\d+)*$/.test(text)) return '';
  return text;
}

function arrayValue(value) {
  return Array.isArray(value) ? value : [];
}

function dateTimeValue(value) {
  if (value === null || value === undefined || value === '') return 0;
  const timestamp = value instanceof Date ? value.getTime() : new Date(value).getTime();
  return Number.isFinite(timestamp) ? timestamp : 0;
}

function documentUpdatedAtValue(document = {}) {
  return dateTimeValue(
    document.updated_at
      || document.updatedAt
      || document.created_at
      || document.createdAt,
  );
}

export function documentUpdatedAt(document = {}) {
  const timestamp = documentUpdatedAtValue(document);
  return timestamp > 0 ? new Date(timestamp).toISOString() : '';
}

export function documentDatasetIds(document = {}) {
  return uniqueTexts([
    document.dataset_id,
    document.datasetId,
    ...arrayValue(document.dataset_ids),
    ...arrayValue(document.datasetIds),
  ]);
}

export function sourceFieldHints(source = {}, { limit = FIELD_HINT_LIMIT } = {}) {
  const safeLimit = Number.isFinite(Number(limit)) && Number(limit) >= 0
    ? Math.floor(Number(limit))
    : FIELD_HINT_LIMIT;
  if (!safeLimit) return [];

  return uniqueTexts([
    ...arrayValue(source.noun_term_hints),
    ...arrayValue(source.nounTermHints),
    ...arrayValue(source.section_title_hints),
    ...arrayValue(source.sectionTitleHints),
    ...arrayValue(source.material_hints),
    ...arrayValue(source.materialHints),
  ].map(fieldHintText).filter(Boolean), safeLimit);
}

export function sourceDisplayName(source = {}, aliases = {}) {
  const sourceId = normalizedText(source.id || source.dataset_id || source.datasetId);
  const alias = aliases instanceof Map
    ? aliases.get(sourceId)
    : sourceId && aliases && typeof aliases === 'object'
      && Object.prototype.hasOwnProperty.call(aliases, sourceId)
      ? aliases[sourceId]
      : '';
  return normalizedText(alias) || normalizedText(source.title);
}

function datasetId(dataset = {}) {
  return normalizedText(dataset.id || dataset.dataset_id || dataset.datasetId);
}

function datasetTitle(dataset = {}, id = '') {
  return normalizedText(dataset.title || dataset.name || dataset.key) || id;
}

function documentIdentity(document = {}, fallbackIndex = 0) {
  const stableIdentity = normalizedText(
    document.id
      || document.document_id
      || document.documentId
      || document.external_id
      || document.externalId
      || document.object_key
      || document.objectKey,
  );
  return stableIdentity ? `document:${stableIdentity}` : `anonymous:${fallbackIndex}`;
}

function mergeDocumentRecords(existing, incoming, incomingDatasetIds) {
  incomingDatasetIds.forEach((id) => existing.datasetIds.add(id));

  const existingTime = documentUpdatedAtValue(existing.document);
  const incomingTime = documentUpdatedAtValue(incoming);
  const incomingIsPreferred = incomingTime >= existingTime;
  existing.document = incomingIsPreferred
    ? { ...existing.document, ...incoming }
    : { ...incoming, ...existing.document };
}

function collectDocumentRecords(datasets, documents) {
  const records = new Map();
  let fallbackIndex = 0;

  const addDocument = (document, forcedDatasetIds = []) => {
    if (!document || typeof document !== 'object' || Array.isArray(document)) return;
    const memberDatasetIds = uniqueTexts([
      ...documentDatasetIds(document),
      ...forcedDatasetIds,
    ]);
    if (!memberDatasetIds.length) return;

    const key = documentIdentity(document, fallbackIndex);
    fallbackIndex += 1;
    const existing = records.get(key);
    if (existing) {
      mergeDocumentRecords(existing, document, memberDatasetIds);
      return;
    }
    records.set(key, {
      document: { ...document },
      datasetIds: new Set(memberDatasetIds),
    });
  };

  documents.forEach((document) => addDocument(document));
  datasets.forEach((dataset) => {
    const id = datasetId(dataset);
    if (!id) return;
    arrayValue(dataset.documents).forEach((document) => addDocument(document, [id]));
  });
  return records;
}

export function connectedDocumentCount(datasets = [], documents = []) {
  const visibleDatasetIds = new Set(
    (Array.isArray(datasets) ? datasets : []).map(datasetId).filter(Boolean),
  );
  if (!visibleDatasetIds.size) return 0;

  let count = 0;
  collectDocumentRecords(
    Array.isArray(datasets) ? datasets.filter(Boolean) : [],
    Array.isArray(documents) ? documents.filter(Boolean) : [],
  ).forEach(({ datasetIds }) => {
    if ([...datasetIds].some((id) => visibleDatasetIds.has(id))) count += 1;
  });
  return count;
}

function numericValue(value) {
  if (value === null || value === undefined || value === '') return null;
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, number) : null;
}

function documentWordCount(document = {}) {
  const candidates = [
    document.estimated_word_count,
    document.estimatedWordCount,
    document.word_count,
    document.wordCount,
  ];
  for (const candidate of candidates) {
    const value = numericValue(candidate);
    if (value !== null) return value;
  }
  return 0;
}

function sourceEstimatedWordCount(source = {}, documents = []) {
  const candidates = [
    source.estimated_word_count,
    source.estimatedWordCount,
    source.word_count,
    source.wordCount,
  ];
  for (const candidate of candidates) {
    const value = numericValue(candidate);
    if (value !== null) return value;
  }
  return documents.reduce((sum, document) => sum + documentWordCount(document), 0);
}

function documentContentType(document = {}) {
  return normalizedText(document.content_type || document.contentType);
}

function documentParseStatus(document = {}) {
  return normalizedText(document.parse_status || document.parseStatus || document.status) || 'unknown';
}

function buildParseCounts(documents = []) {
  return documents.reduce((counts, document) => {
    const status = documentParseStatus(document);
    counts[status] = (counts[status] || 0) + 1;
    return counts;
  }, {});
}

function normalizeNow(now) {
  const timestamp = dateTimeValue(now);
  return timestamp > 0 ? timestamp : Date.now();
}

export function buildConnectedSourceCards(datasets = [], documents = [], options = {}) {
  const visibleDatasets = Array.isArray(datasets) ? datasets.filter(Boolean) : [];
  const visibleDocuments = Array.isArray(documents) ? documents.filter(Boolean) : [];
  const nowTimestamp = normalizeNow(options?.now);
  const datasetOrder = new Map();
  const datasetsById = new Map();

  visibleDatasets.forEach((dataset, index) => {
    const id = datasetId(dataset);
    if (!id || datasetsById.has(id)) return;
    datasetOrder.set(id, index);
    datasetsById.set(id, dataset);
  });

  const documentsByDatasetId = new Map(
    [...datasetsById.keys()].map((id) => [id, []]),
  );
  const documentRecords = collectDocumentRecords(visibleDatasets, visibleDocuments);
  documentRecords.forEach(({ document, datasetIds }) => {
    datasetIds.forEach((id) => {
      const groupedDocuments = documentsByDatasetId.get(id);
      if (groupedDocuments) groupedDocuments.push(document);
    });
  });

  return [...datasetsById.entries()]
    .map(([id, dataset]) => {
      const groupedDocuments = documentsByDatasetId.get(id) || [];
      const recentDocuments = [...groupedDocuments]
        .sort((left, right) => documentUpdatedAtValue(right) - documentUpdatedAtValue(left))
        .slice(0, RECENT_DOCUMENT_LIMIT);
      const latestTimestamp = recentDocuments.length
        ? documentUpdatedAtValue(recentDocuments[0])
        : 0;
      const recentThreshold = nowTimestamp - RECENT_WINDOW_MS;

      return {
        id,
        title: datasetTitle(dataset, id),
        documentCount: groupedDocuments.length,
        estimatedWordCount: sourceEstimatedWordCount(dataset, groupedDocuments),
        fieldHints: sourceFieldHints(dataset),
        contentTypes: uniqueTexts(groupedDocuments.map(documentContentType)),
        parseCounts: buildParseCounts(groupedDocuments),
        latestUpdatedAt: latestTimestamp > 0 ? new Date(latestTimestamp).toISOString() : '',
        recent24hCount: groupedDocuments.filter((document) => {
          const timestamp = documentUpdatedAtValue(document);
          return timestamp >= recentThreshold && timestamp <= nowTimestamp;
        }).length,
        recentDocuments,
      };
    })
    .sort((left, right) => {
      const changeDifference = dateTimeValue(right.latestUpdatedAt) - dateTimeValue(left.latestUpdatedAt);
      if (changeDifference) return changeDifference;
      return (datasetOrder.get(left.id) || 0) - (datasetOrder.get(right.id) || 0);
    });
}
