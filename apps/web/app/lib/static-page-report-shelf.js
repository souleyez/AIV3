import { normalizeDatasetIds, staticPageDraftDatasetIds } from './dataset-record-scope.js';

export function staticPageDraftArtifactKey(draft) {
  return String(
    draft?.source_refs?.artifact_stability?.dataset_artifact_key
      || draft?.source_refs?.artifact_stability?.datasetArtifactKey
      || draft?.sourceRefs?.artifactStability?.datasetArtifactKey
      || draft?.sourceRefs?.artifact_stability?.dataset_artifact_key
      || draft?.artifact_stability?.dataset_artifact_key
      || draft?.artifactStability?.datasetArtifactKey
      || draft?.draft_payload?.artifact_stability?.dataset_artifact_key
      || draft?.draft_payload?.artifactStability?.datasetArtifactKey
      || draft?.draftPayload?.artifactStability?.datasetArtifactKey
      || draft?.draftPayload?.artifact_stability?.dataset_artifact_key
      || draft?.source_refs?.dataset_artifact_key
      || draft?.sourceRefs?.datasetArtifactKey
      || draft?.dataset_artifact_key
      || draft?.datasetArtifactKey
      || '',
  ).trim();
}

export function staticPageDraftBaselineStatus(draft) {
  return String(
    draft?.source_refs?.artifact_stability?.baseline_status
      || draft?.source_refs?.artifact_stability?.baselineStatus
      || draft?.sourceRefs?.artifactStability?.baselineStatus
      || draft?.sourceRefs?.artifact_stability?.baseline_status
      || draft?.artifact_stability?.baseline_status
      || draft?.artifactStability?.baselineStatus
      || draft?.draft_payload?.artifact_stability?.baseline_status
      || draft?.draft_payload?.artifactStability?.baselineStatus
      || draft?.draftPayload?.artifactStability?.baselineStatus
      || draft?.draftPayload?.artifact_stability?.baseline_status
      || draft?.finalPage?.baselineStatus
      || draft?.finalPage?.baseline_status
      || '',
  ).trim().toLowerCase();
}

export function normalizeReportShelfDefaultValue(value) {
  if (typeof value === 'boolean') {
    return value;
  }
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase();
    if (['true', '1', 'yes', 'default', 'accepted', 'enabled'].includes(normalized)) {
      return true;
    }
    if (['false', '0', 'no', 'not_default', 'non_default', 'retired', 'disabled'].includes(normalized)) {
      return false;
    }
  }
  return null;
}

function mergeReportShelfDefaultEntries(target, value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return target;
  }
  Object.entries(value).forEach(([datasetId, enabled]) => {
    const normalizedDatasetId = String(datasetId || '').trim();
    const normalizedValue = normalizeReportShelfDefaultValue(enabled);
    if (normalizedDatasetId && normalizedValue !== null) {
      target[normalizedDatasetId] = normalizedValue;
    }
  });
  return target;
}

export function staticPageDraftReportShelfDefaults(draft) {
  const defaults = {};
  [
    draft?.source_refs?.artifact_stability?.report_shelf_defaults,
    draft?.source_refs?.artifact_stability?.reportShelfDefaults,
    draft?.sourceRefs?.artifactStability?.reportShelfDefaults,
    draft?.sourceRefs?.artifact_stability?.report_shelf_defaults,
    draft?.source_refs?.report_shelf_defaults,
    draft?.sourceRefs?.reportShelfDefaults,
    draft?.artifact_stability?.report_shelf_defaults,
    draft?.artifactStability?.reportShelfDefaults,
    draft?.draft_payload?.artifact_stability?.report_shelf_defaults,
    draft?.draft_payload?.artifactStability?.reportShelfDefaults,
    draft?.draftPayload?.artifactStability?.reportShelfDefaults,
    draft?.draftPayload?.artifact_stability?.report_shelf_defaults,
    draft?.reportShelfDefaults,
    draft?.report_shelf_defaults,
  ].forEach((value) => mergeReportShelfDefaultEntries(defaults, value));
  return defaults;
}

export function staticPageDraftIsDefaultForDatasetIds(draft, datasetIds) {
  const ids = normalizeDatasetIds(datasetIds);
  if (!draft || !ids.length) {
    return false;
  }
  const overrides = staticPageDraftReportShelfDefaults(draft);
  const relatedIdSet = new Set(staticPageDraftDatasetIds(draft));
  return ids.some((datasetId) => {
    if (Object.prototype.hasOwnProperty.call(overrides, datasetId)) {
      return overrides[datasetId] === true;
    }
    return staticPageDraftBaselineStatus(draft) !== 'retired' && relatedIdSet.has(datasetId);
  });
}

export function staticPageDraftHasAnyReportShelfDefault(draft) {
  if (!draft) {
    return false;
  }
  const overrides = staticPageDraftReportShelfDefaults(draft);
  if (Object.values(overrides).some((enabled) => enabled === true)) {
    return true;
  }
  if (staticPageDraftBaselineStatus(draft) === 'retired') {
    return false;
  }
  return staticPageDraftDatasetIds(draft).some((datasetId) => overrides[datasetId] !== false);
}

export function reportShelfDefaultTargetDatasetIds(draft, selectedDatasetIds) {
  const selectedIds = normalizeDatasetIds(selectedDatasetIds);
  if (selectedIds.length) {
    return selectedIds;
  }
  return staticPageDraftDatasetIds(draft);
}

export function withStaticPageDraftReportShelfDefaults(draft, datasetIds, enabled) {
  const targetDatasetIds = normalizeDatasetIds(datasetIds);
  if (!draft || !targetDatasetIds.length) {
    return draft;
  }
  const nextDefaults = {
    ...staticPageDraftReportShelfDefaults(draft),
  };
  targetDatasetIds.forEach((datasetId) => {
    nextDefaults[datasetId] = Boolean(enabled);
  });

  const sourceRefs = {
    ...(draft.source_refs || draft.sourceRefs || {}),
  };
  const sourceStability = {
    ...(sourceRefs.artifact_stability || sourceRefs.artifactStability || {}),
    report_shelf_defaults: nextDefaults,
    reportShelfDefaults: nextDefaults,
    report_shelf_defaults_updated_at: new Date().toISOString(),
  };
  if (enabled) {
    sourceStability.baseline_status = 'accepted';
    sourceStability.baselineStatus = 'accepted';
  }
  sourceRefs.artifact_stability = sourceStability;
  sourceRefs.artifactStability = sourceStability;

  return {
    ...draft,
    source_refs: sourceRefs,
    sourceRefs,
    reportShelfDefaults: nextDefaults,
    report_shelf_defaults: nextDefaults,
  };
}
