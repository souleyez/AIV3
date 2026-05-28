export function documentRawTextFromChunks(chunks = []) {
  return chunks
    .map((chunk) => String(chunk?.content || '').trim())
    .filter(Boolean)
    .join('\n\n');
}

export function chunkSectionHints(chunk) {
  const hints = chunk?.metadata?.section_title_hints;
  if (Array.isArray(hints)) {
    return hints.map((item) => String(item || '').trim()).filter(Boolean);
  }
  return [];
}

function documentDatasetIds(document) {
  return [...new Set([
    document?.dataset_id,
    document?.datasetId,
    ...(Array.isArray(document?.dataset_ids) ? document.dataset_ids : []),
    ...(Array.isArray(document?.datasetIds) ? document.datasetIds : []),
  ].map((id) => String(id || '').trim()).filter(Boolean))];
}

export function buildDocumentDetailViewModel({
  documents = [],
  selectedDocumentId = '',
  selectedDocumentDetail = null,
} = {}) {
  const chunks = Array.isArray(selectedDocumentDetail?.chunks) ? selectedDocumentDetail.chunks : [];
  const orderedChunks = [...chunks].sort((left, right) => (left.chunk_index || 0) - (right.chunk_index || 0));
  const evidences = Array.isArray(selectedDocumentDetail?.retrieval_evidences)
    ? selectedDocumentDetail.retrieval_evidences
    : [];
  const selectedDocument = selectedDocumentDetail?.document
    || documents.find((document) => document.id === selectedDocumentId)
    || null;
  const selectedDatasetIds = new Set(documentDatasetIds(selectedDocument));
  const siblingDocuments = selectedDatasetIds.size
    ? documents.filter((document) => documentDatasetIds(document).some((datasetId) => selectedDatasetIds.has(datasetId)))
    : documents;
  const currentIndex = siblingDocuments.findIndex((document) => document.id === selectedDocument?.id);
  const previousDocument = currentIndex > 0 ? siblingDocuments[currentIndex - 1] : null;
  const nextDocument = currentIndex >= 0 && currentIndex < siblingDocuments.length - 1
    ? siblingDocuments[currentIndex + 1]
    : null;
  const rawText = documentRawTextFromChunks(orderedChunks);
  const markdownSectionHints = [...new Set(orderedChunks.flatMap(chunkSectionHints))].slice(0, 24);
  const modelFacing = selectedDocumentDetail?.model_facing || null;

  return {
    orderedChunks,
    evidences,
    selectedDocument,
    previousDocument,
    nextDocument,
    rawText,
    markdownSectionHints,
    modelFacing,
  };
}
