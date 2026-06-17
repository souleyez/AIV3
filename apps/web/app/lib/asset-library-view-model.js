export const ASSET_LIBRARY_PRESETS = Object.freeze([
  Object.freeze({
    id: 'fashion_design',
    label: '服装设计样例',
    name: '服装设计资产库',
    domain: 'fashion_design',
    description: '统一管理灵感图、设计稿、款式文档、供应链资料、营销产物和报表产物。',
    metadata: Object.freeze({
      preset_id: 'fashion_design',
      sample_domain: 'fashion_design',
      recommended_collections: Object.freeze([
        '灵感图库',
        '设计稿',
        '款式文档',
        '供应链资料',
        '营销产物',
        '报表产物',
      ]),
    }),
  }),
]);

export function normalizeAssetLibraries(payload) {
  const items = Array.isArray(payload)
    ? payload
    : Array.isArray(payload?.asset_libraries)
      ? payload.asset_libraries
      : Array.isArray(payload?.assetLibraries)
        ? payload.assetLibraries
        : [];
  return items
    .map((item) => ({
      id: String(item?.id || '').trim(),
      externalId: String(item?.external_id || item?.externalId || '').trim(),
      name: String(item?.name || '').trim(),
      domain: String(item?.domain || 'general').trim() || 'general',
      description: String(item?.description || '').trim(),
      visibility: String(item?.visibility || 'private').trim() || 'private',
      datasetCount: Number(item?.dataset_count ?? item?.datasetCount ?? 0) || 0,
      metadata: item?.metadata && typeof item.metadata === 'object' ? item.metadata : {},
      createdAt: item?.created_at || item?.createdAt || '',
      updatedAt: item?.updated_at || item?.updatedAt || '',
    }))
    .filter((item) => item.id && item.name)
    .sort((left, right) => {
      const leftTime = new Date(left.updatedAt || left.createdAt || 0).getTime();
      const rightTime = new Date(right.updatedAt || right.createdAt || 0).getTime();
      if (Number.isFinite(leftTime) && Number.isFinite(rightTime) && leftTime !== rightTime) {
        return rightTime - leftTime;
      }
      return left.name.localeCompare(right.name, 'zh-Hans-CN');
    });
}

export function assetLibraryPresetById(presetId) {
  const id = String(presetId || '').trim();
  return ASSET_LIBRARY_PRESETS.find((preset) => preset.id === id) || null;
}

export function applyAssetLibraryPresetToDraft(draft, presetId) {
  const preset = assetLibraryPresetById(presetId);
  if (!preset) {
    return { ...(draft || {}) };
  }
  return {
    ...(draft || {}),
    name: preset.name,
    domain: preset.domain,
    description: preset.description,
    presetId: preset.id,
    metadata: { ...(preset.metadata || {}) },
  };
}

export function buildAssetLibraryCreatePayload(draft) {
  const metadata = {
    created_from: 'main_workspace',
    ...((draft?.metadata && typeof draft.metadata === 'object') ? draft.metadata : {}),
  };
  if (draft?.presetId) {
    metadata.preset_id = String(draft.presetId).trim();
  }
  return {
    name: String(draft?.name || '').trim(),
    domain: String(draft?.domain || 'general').trim() || 'general',
    description: String(draft?.description || '').trim() || undefined,
    visibility: String(draft?.visibility || 'private').trim() || 'private',
    metadata,
  };
}

export function normalizeAssetLibraryScope(payload) {
  const summary = payload?.summary || payload || {};
  const datasets = Array.isArray(summary.datasets) ? summary.datasets : [];
  const memberships = Array.isArray(summary.memberships) ? summary.memberships : [];
  const assets = Array.isArray(summary.assets) ? summary.assets : [];
  const assetProfileHints = Array.isArray(summary.asset_profile_hints)
    ? summary.asset_profile_hints
    : Array.isArray(summary.assetProfileHints)
      ? summary.assetProfileHints
      : [];
  const normalizedAssetProfileHints = assetProfileHints.map(normalizeAssetProfileHint).filter(Boolean);
  return {
    assetLibrary: summary.asset_library || summary.assetLibrary || null,
    datasets,
    memberships,
    assets,
    assetProfileHints: normalizedAssetProfileHints,
    datasetIds: normalizeStringIds(summary.dataset_ids || summary.datasetIds),
    deniedDatasetCount: Number(summary.denied_dataset_count ?? summary.deniedDatasetCount ?? 0) || 0,
    membershipCount: Number(summary.membership_count ?? summary.membershipCount ?? memberships.length) || 0,
    authorizedDatasetCount: Number(summary.authorized_dataset_count ?? summary.authorizedDatasetCount ?? datasets.length) || 0,
    assetCount: Number(summary.asset_count ?? summary.assetCount ?? assets.length) || 0,
    assetProfileHintCount: Number(summary.asset_profile_hint_count ?? summary.assetProfileHintCount ?? normalizedAssetProfileHints.length) || 0,
    scopePolicy: String(summary.scope_policy || summary.scopePolicy || '').trim(),
  };
}

export function normalizeAssetProfileHint(hint) {
  if (!hint || typeof hint !== 'object') return null;
  const assetId = String(hint.asset_id || hint.assetId || '').trim();
  const title = String(hint.title || hint.name || assetId || '').trim();
  const summary = String(hint.summary || '').trim();
  const assetKind = String(hint.asset_kind || hint.assetKind || 'asset').trim() || 'asset';
  const sourceKind = String(hint.source_kind || hint.sourceKind || '').trim();
  const profileKind = String(hint.profile_kind || hint.profileKind || '').trim();
  const nounTerms = normalizeTextList(hint.noun_terms || hint.nounTerms);
  const facets = normalizeTextList(hint.facets);
  if (!assetId && !title && !summary && !nounTerms.length && !facets.length) return null;
  return {
    assetId,
    title,
    summary,
    assetKind,
    sourceKind,
    profileKind,
    nounTerms,
    facets,
    searchText: [
      assetId,
      title,
      summary,
      assetKind,
      sourceKind,
      profileKind,
      ...nounTerms,
      ...facets,
    ].join(' ').toLowerCase(),
  };
}

export function assetProfileKindOptions(scope) {
  const kinds = new Set();
  for (const hint of scope?.assetProfileHints || []) {
    if (hint?.assetKind) kinds.add(hint.assetKind);
  }
  return [...kinds].sort((left, right) => left.localeCompare(right, 'zh-Hans-CN'));
}

export function filterAssetProfileHints(scope, options = {}) {
  const query = String(options.query || '').trim().toLowerCase();
  const assetKind = String(options.assetKind || '').trim();
  const limit = Number(options.limit || 8) || 8;
  return (scope?.assetProfileHints || [])
    .filter((hint) => {
      if (assetKind && assetKind !== 'all' && hint.assetKind !== assetKind) return false;
      if (query && !hint.searchText.includes(query)) return false;
      return true;
    })
    .slice(0, Math.max(1, Math.min(limit, 50)));
}

export function assetLibraryContainsDataset(scope, datasetId) {
  const id = String(datasetId || '').trim();
  if (!id) return false;
  return normalizeStringIds(scope?.datasetIds || scope?.dataset_ids).includes(id)
    || (Array.isArray(scope?.datasets) && scope.datasets.some((dataset) => String(dataset?.id || '').trim() === id))
    || (Array.isArray(scope?.memberships) && scope.memberships.some((membership) => (
      String(membership?.dataset_id || membership?.datasetId || '').trim() === id
    )));
}

export function selectedAssetLibraryView(assetLibraries, selectedAssetLibraryId) {
  const id = String(selectedAssetLibraryId || '').trim();
  if (!id) return null;
  return (assetLibraries || []).find((item) => item.id === id) || null;
}

function normalizeStringIds(values) {
  return [...new Set((Array.isArray(values) ? values : [])
    .map((value) => String(value || '').trim())
    .filter(Boolean))];
}

function normalizeTextList(values) {
  return [...new Set((Array.isArray(values) ? values : [])
    .map((value) => String(value || '').trim())
    .filter(Boolean))]
    .slice(0, 24);
}
