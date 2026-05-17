export function attachVisibleDocumentsToDatasets(datasets = [], documents = []) {
  const visibleDatasets = Array.isArray(datasets) ? datasets : [];
  const visibleDocuments = Array.isArray(documents) ? documents : [];
  if (!visibleDatasets.length || !visibleDocuments.length) {
    return visibleDatasets;
  }
  const documentsByDatasetId = visibleDocuments.reduce((map, document) => {
    const datasetId = datasetIdForDocument(document);
    if (!datasetId) return map;
    const current = map.get(datasetId) || [];
    current.push(document);
    map.set(datasetId, current);
    return map;
  }, new Map());

  return visibleDatasets.map((dataset) => {
    const datasetId = String(dataset?.id || '').trim();
    const attached = datasetId ? documentsByDatasetId.get(datasetId) || [] : [];
    const existing = Array.isArray(dataset?.documents) ? dataset.documents : [];
    if (!attached.length && !existing.length) {
      return dataset;
    }
    return {
      ...dataset,
      documents: dedupeDocuments([...existing, ...attached]),
    };
  });
}

export function datasetDocumentTitleHints(dataset = {}, { limit = 6 } = {}) {
  const hints = [];
  const configuredHints = Array.isArray(dataset.documentTitleHints)
    ? dataset.documentTitleHints
    : Array.isArray(dataset.document_title_hints)
      ? dataset.document_title_hints
      : [];
  hints.push(...configuredHints);
  if (Array.isArray(dataset.documents)) {
    hints.push(...dataset.documents.map(documentTitleCandidate));
  }
  const seen = new Set();
  return hints
    .map(normalizeDocumentTitleHint)
    .filter(Boolean)
    .filter((hint) => {
      if (seen.has(hint)) return false;
      seen.add(hint);
      return true;
    })
    .slice(0, limit);
}

function dedupeDocuments(documents) {
  const seen = new Set();
  return documents.filter((document) => {
    const key = String(
      document?.id
        || document?.document_id
        || document?.documentId
        || document?.object_key
        || document?.objectKey
        || document?.title
        || '',
    ).trim();
    if (!key) return true;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function datasetIdForDocument(document) {
  return String(document?.dataset_id || document?.datasetId || '').trim();
}

function documentTitleCandidate(document) {
  return document?.title
    || document?.name
    || document?.filename
    || document?.object_key
    || document?.objectKey
    || '';
}

function normalizeDocumentTitleHint(value) {
  const raw = String(value || '').trim();
  if (!raw) return '';
  const filename = raw.split(/[\\/]/).filter(Boolean).pop() || raw;
  const withoutQuery = filename.split(/[?#]/)[0] || filename;
  const withoutExtension = withoutQuery.replace(/\.[A-Za-z0-9]{1,12}$/, '');
  return withoutExtension
    .trim()
    .replace(/^[._\-\s]+|[._\-\s]+$/g, '')
    .slice(0, 80);
}
