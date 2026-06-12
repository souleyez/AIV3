export function sortByDateDesc(items, fieldName) {
  return [...(Array.isArray(items) ? items : [])].sort((left, right) => {
    const leftValue = new Date(left?.[fieldName] || 0).getTime();
    const rightValue = new Date(right?.[fieldName] || 0).getTime();
    return rightValue - leftValue;
  });
}

export function sortDatasets(items) {
  return [...(Array.isArray(items) ? items : [])].sort((left, right) =>
    String(left?.title || left?.key || '').localeCompare(String(right?.title || right?.key || ''), 'zh-CN'),
  );
}

export function normalizeDatasetIds(ids) {
  return [...new Set((Array.isArray(ids) ? ids : [ids])
    .map((id) => String(id || '').trim())
    .filter(Boolean))];
}

export function toggleSelectedDatasetIds(currentDatasetIds, datasetId) {
  const currentIds = normalizeDatasetIds(currentDatasetIds);
  return currentIds.includes(datasetId)
    ? currentIds.filter((item) => item !== datasetId)
    : [...currentIds, datasetId];
}

export function datasetSelectionStateAfterToggle(currentDatasetIds, datasetId) {
  const selectedDatasetIds = toggleSelectedDatasetIds(currentDatasetIds, datasetId);
  return {
    selectedDatasetId: selectedDatasetIds[0] || null,
    selectedDatasetIds,
  };
}

function datasetIdSet(items) {
  return new Set((Array.isArray(items) ? items : []).map((item) => item?.id).filter(Boolean));
}

export function datasetIdsAfterCatalogRefresh(currentDatasetIds, nextDatasets, preferredDatasetId = null) {
  const availableIds = datasetIdSet(nextDatasets);
  const retained = normalizeDatasetIds(currentDatasetIds).filter((datasetId) => availableIds.has(datasetId));
  if (preferredDatasetId && availableIds.has(preferredDatasetId)) {
    return normalizeDatasetIds([preferredDatasetId, ...retained]);
  }
  return retained;
}

export function selectedDatasetIdAfterCatalogRefresh(currentDatasetId, nextDatasets, preferredDatasetId = null) {
  const availableIds = datasetIdSet(nextDatasets);
  if (preferredDatasetId && availableIds.has(preferredDatasetId)) {
    return preferredDatasetId;
  }
  if (currentDatasetId && availableIds.has(currentDatasetId)) {
    return currentDatasetId;
  }
  return null;
}

export function documentDatasetIds(document) {
  return normalizeDatasetIds([
    document?.dataset_id,
    document?.datasetId,
    ...(Array.isArray(document?.dataset_ids) ? document.dataset_ids : []),
    ...(Array.isArray(document?.datasetIds) ? document.datasetIds : []),
  ]);
}

export function documentMembershipCurrentDatasetIds(document, fallbackDatasetIds = []) {
  const documentIds = documentDatasetIds(document);
  return documentIds.length ? documentIds : normalizeDatasetIds(fallbackDatasetIds);
}

export function documentMembershipToggleIntent(document, fallbackDatasetIds, datasetId) {
  const currentDatasetIds = documentMembershipCurrentDatasetIds(document, fallbackDatasetIds);
  const active = currentDatasetIds.includes(datasetId);
  return {
    active,
    method: active ? 'DELETE' : 'PUT',
    banner: active ? '已将文档移出该数据集。' : '已将文档加入该数据集。',
    currentDatasetIds,
  };
}

export function documentDatasetSelectionUpdate(document) {
  const datasetIds = documentDatasetIds(document);
  if (!datasetIds.length) {
    return null;
  }
  return {
    selectedDatasetId: datasetIds[0],
    selectedDatasetIds: datasetIds,
  };
}

export function documentMembershipResponseDatasetIds(response) {
  return normalizeDatasetIds(
    response?.dataset_ids
      || response?.datasetIds
      || response?.document?.dataset_ids
      || response?.document?.datasetIds
      || [],
  );
}

export function documentMembershipResponseSelectionUpdate(response) {
  const selectedDatasetIds = documentMembershipResponseDatasetIds(response);
  if (!selectedDatasetIds.length) {
    return null;
  }
  return {
    selectedDatasetId: selectedDatasetIds[0],
    selectedDatasetIds,
  };
}

export function reportRecordDatasetIds(record) {
  return normalizeDatasetIds([
    record?.dataset_id,
    record?.datasetId,
    record?.dataset?.id,
    record?.plan?.dataset_id,
    record?.plan?.datasetId,
    ...(Array.isArray(record?.dataset_ids) ? record.dataset_ids : []),
    ...(Array.isArray(record?.datasetIds) ? record.datasetIds : []),
  ]);
}

export function staticPageDraftDatasetIds(draft) {
  return normalizeDatasetIds([
    ...(Array.isArray(draft?.matchedDatasetIds) ? draft.matchedDatasetIds : []),
    ...(Array.isArray(draft?.matched_dataset_ids) ? draft.matched_dataset_ids : []),
    draft?.matchedDatasetId,
    draft?.matched_dataset_id,
    draft?.datasetId,
    draft?.dataset_id,
    draft?.dataSnapshot?.datasetId,
    draft?.dataSnapshot?.dataset_id,
    draft?.source?.datasetId,
    draft?.source?.dataset_id,
    draft?.source_refs?.dataset_id,
    draft?.sourceRefs?.datasetId,
  ]);
}

export function filterRecordsByDatasetIds(items, datasetIds, ownerIds = reportRecordDatasetIds) {
  const idSet = new Set(normalizeDatasetIds(datasetIds));
  if (!idSet.size) {
    return [];
  }
  return (Array.isArray(items) ? items : []).filter((item) =>
    ownerIds(item).some((datasetId) => idSet.has(datasetId)),
  );
}

export function sameDatasetIds(left, right) {
  const leftIds = normalizeDatasetIds(left);
  const rightIds = normalizeDatasetIds(right);
  return leftIds.length === rightIds.length && leftIds.every((id, index) => id === rightIds[index]);
}
