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

export function normalizeAssetImportReadiness(payload) {
  return {
    enabled: payload?.enabled === true,
    reasonCode: String(payload?.reason_code || payload?.reasonCode || 'not_ready').trim() || 'not_ready',
    singleSupported: payload?.single_supported === true || payload?.singleSupported === true,
    batchSupported: payload?.batch_supported === true || payload?.batchSupported === true,
    zipSupported: payload?.zip_supported === true || payload?.zipSupported === true,
  };
}

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

export function buildFashionDesignImageAssetImportPayload(draft = {}, context = {}) {
  const datasetId = trimmedString(draft.datasetId || draft.dataset_id || context.datasetId || context.dataset_id);
  const assetLibraryId = trimmedString(
    draft.assetLibraryId || draft.asset_library_id || context.assetLibraryId || context.asset_library_id,
  );
  const collectionId = trimmedString(draft.collectionId || draft.collection_id);
  const externalId = trimmedString(draft.externalId || draft.external_id);
  const imageUrl = trimmedString(draft.imageUrl || draft.image_url || draft.url);
  const objectKey = trimmedString(draft.objectKey || draft.object_key);
  const source = externalId || objectKey || imageUrl;
  const title = trimmedString(draft.title || inferAssetTitleFromSource(imageUrl || objectKey || externalId));
  const errors = [];

  if (!datasetId) errors.push('请选择要写入的目标数据集。');
  if (!source) errors.push('请输入图片 URL、对象 key 或外部 ID。');
  if (!title) errors.push('请输入图片标题。');
  if (collectionId && !assetLibraryId) errors.push('请选择资产库后再选择图库分组。');

  const parsedProfile = parseJsonObjectDraft(
    draft.profilePayload ?? draft.profile_payload,
    draft.profilePayloadText ?? draft.profile_payload_text,
    '画像 JSON',
  );
  if (parsedProfile.error) errors.push(parsedProfile.error);

  const parsedMetadata = parseJsonObjectDraft(
    draft.metadata,
    draft.metadataText ?? draft.metadata_text,
    '元数据 JSON',
  );
  if (parsedMetadata.error) errors.push(parsedMetadata.error);

  const metadata = {
    imported_from: 'main_workspace_asset_library',
    ...sanitizeAssetImportMetadata(parsedMetadata.value || {}),
  };
  if (assetLibraryId) metadata.asset_library_id = assetLibraryId;

  const payload = {
    dataset_id: datasetId,
    ...(assetLibraryId ? { asset_library_id: assetLibraryId } : {}),
    ...(collectionId ? { collection_id: collectionId } : {}),
    ...(externalId ? { external_id: externalId } : {}),
    title,
    ...(imageUrl ? { image_url: imageUrl } : {}),
    ...(objectKey ? { object_key: objectKey } : {}),
    ...(trimmedString(draft.contentType || draft.content_type) || inferImageContentType(imageUrl || objectKey)
      ? { content_type: trimmedString(draft.contentType || draft.content_type) || inferImageContentType(imageUrl || objectKey) }
      : {}),
    profile_payload: parsedProfile.value || {},
    metadata,
  };

  return { payload, errors };
}

export function buildFashionDesignImageAssetImportBatchPayload(draft = {}, context = {}) {
  const datasetId = trimmedString(draft.datasetId || draft.dataset_id || context.datasetId || context.dataset_id);
  const assetLibraryId = trimmedString(
    draft.assetLibraryId || draft.asset_library_id || context.assetLibraryId || context.asset_library_id,
  );
  const collectionId = trimmedString(draft.collectionId || draft.collection_id);
  const sources = parseAssetImportSources(draft.imageUrlsText || draft.image_urls_text || draft.imageUrl || draft.image_url || draft.url);
  const packages = normalizeAssetImportPackages(draft.packages || draft.assetPackages || draft.asset_import_packages);
  const errors = [];

  if (!datasetId) errors.push('请选择要写入的目标数据集。');
  if (!sources.length && !packages.length) errors.push('请输入至少一个图片 URL、对象 key 或 ZIP 包。');
  if (collectionId && !assetLibraryId) errors.push('请选择资产库后再选择图库分组。');

  const parsedProfile = parseJsonObjectDraft(
    draft.profilePayload ?? draft.profile_payload,
    draft.profilePayloadText ?? draft.profile_payload_text,
    '画像 JSON',
  );
  if (parsedProfile.error) errors.push(parsedProfile.error);

  const parsedMetadata = parseJsonObjectDraft(
    draft.metadata,
    draft.metadataText ?? draft.metadata_text,
    '元数据 JSON',
  );
  if (parsedMetadata.error) errors.push(parsedMetadata.error);

  const baseTitle = trimmedString(draft.title);
  const baseExternalId = trimmedString(draft.externalId || draft.external_id);
  const payload = {
    dataset_id: datasetId,
    ...(assetLibraryId ? { asset_library_id: assetLibraryId } : {}),
    ...(collectionId ? { collection_id: collectionId } : {}),
    metadata: {
      imported_from: 'main_workspace_asset_library',
      import_mode: 'batch',
      ...sanitizeAssetImportMetadata(parsedMetadata.value || {}),
      ...(assetLibraryId ? { asset_library_id: assetLibraryId } : {}),
    },
    assets: sources.map((source, index) => {
      const imageUrl = looksLikeUrl(source) ? source : '';
      const objectKey = imageUrl ? '' : source;
      const title = sources.length === 1
        ? trimmedString(baseTitle || inferAssetTitleFromSource(source))
        : trimmedString(baseTitle ? `${baseTitle} ${index + 1}` : inferAssetTitleFromSource(source));
      return {
        ...(baseExternalId && sources.length === 1 ? { external_id: baseExternalId } : {}),
        title,
        ...(imageUrl ? { image_url: imageUrl } : {}),
        ...(objectKey ? { object_key: objectKey } : {}),
        ...(inferImageContentType(source) ? { content_type: inferImageContentType(source) } : {}),
        profile_payload: parsedProfile.value || {},
        metadata: {
          import_index: index,
          source_present: true,
          source_kind: imageUrl ? 'image_url' : 'object_key',
        },
      };
    }),
    ...(packages.length ? { packages } : {}),
  };

  if (payload.assets.some((asset) => !asset.title)) {
    errors.push('请输入图片标题，或使用可推断文件名的 URL/object key。');
  }

  return { payload, errors, assetCount: payload.assets.length };
}

export function normalizeFashionDesignImageAssetImportResponse(payload = {}) {
  const rawAsset = payload.asset || payload.asset_item || payload.assetItem || null;
  const rawDatasetMembership = payload.dataset_membership || payload.datasetMembership || null;
  const rawParseRun = payload.parse_run || payload.parseRun || payload.asset_parse_run || payload.assetParseRun || null;
  const rawProfile = payload.profile || payload.asset_profile || payload.assetProfile || null;
  const asset = normalizeAssetItemSummary(rawAsset);
  const datasetMembership = normalizeDatasetAssetMembershipSummary(rawDatasetMembership);
  const parseRun = normalizeAssetParseRunSummary(rawParseRun);
  const profile = normalizeAssetProfileViewSummary(rawProfile);
  return {
    asset,
    datasetMembership,
    parseRun,
    profile,
    assetId: asset?.id || '',
    title: asset?.title || '',
    datasetId: datasetMembership?.datasetId || '',
    parseRunStatus: parseRun?.status || '',
    profileKind: profile?.profileKind || '',
  };
}

export function normalizeFashionDesignImageAssetImportBatchResponse(payload = {}) {
  const items = Array.isArray(payload.items)
    ? payload.items.map(normalizeFashionDesignImageAssetImportResponse)
    : [];
  return {
    accepted: Boolean(payload.accepted ?? items.length),
    assetCount: Number(payload.asset_count ?? payload.assetCount ?? items.length) || 0,
    packageCount: Number(payload.package_count ?? payload.packageCount ?? 0) || 0,
    expandedAssetCount: Number(payload.expanded_asset_count ?? payload.expandedAssetCount ?? 0) || 0,
    items,
  };
}

export function normalizeFashionDesignAssetImportTaskCardDraft(input = {}) {
  const batch = input.batchResponse
    ? normalizeFashionDesignImageAssetImportBatchResponse(input.batchResponse)
    : normalizeFashionDesignImageAssetImportBatchResponse({
      accepted: Boolean(input.response),
      items: input.response ? [input.response] : [],
    });
  const scope = input.scope ? normalizeAssetLibraryScope(input.scope) : normalizeAssetLibraryScope({});
  const request = input.request && typeof input.request === 'object' ? input.request : {};
  const assetLibraryName = trimmedString(input.assetLibraryName || input.asset_library_name || request.asset_library_name);
  const importTitle = trimmedString(input.title || request.title);
  const datasetIdPresent = Boolean(trimmedString(input.datasetId || input.dataset_id || request.dataset_id));
  const assetLibraryIdPresent = Boolean(trimmedString(input.assetLibraryId || input.asset_library_id || request.asset_library_id));
  const importIdPresent = Boolean(trimmedString(input.importId || input.import_id));
  const importIdToken = stableTaskToken(
    input.importId
    || input.import_id
    || batch.items.map((item) => item.assetId).filter(Boolean).join('-')
    || importTitle
    || assetLibraryName
    || 'draft',
  );
  const parseStatusCounts = {
    ...scope.assetParseStatusCounts,
    ...statusCountsFromBatchItems(batch.items),
  };
  const parseEntries = assetParseStatusEntries({
    assetParseStatusCounts: parseStatusCounts,
  }, { limit: 8 });
  const requestedAssetCount = Number(input.requestedAssetCount ?? input.requested_asset_count ?? batch.assetCount ?? batch.items.length) || 0;
  const importedAssetCount = Number(batch.assetCount || batch.items.length || 0) || 0;
  const status = normalizeGalleryTaskStatus(
    input.status
    || input.task_status
    || parseEntries[0]?.status
    || (batch.accepted ? 'queued' : 'pending'),
  );
  const titleSubject = assetLibraryName || importTitle || '设计图资产导入';

  return {
    id: `asset-gallery-import:${importIdToken}`,
    type: 'asset_gallery_import_task',
    title: `图库任务：${titleSubject}`,
    status,
    subtitle: status === 'completed'
      ? '资产画像已可用于筛选、问答和报表供料。'
      : '资产导入与解析状态会在任务卡内持续更新。',
    datasetIdPresent,
    assetLibraryIdPresent,
    importIdPresent,
    requestedAssetCount,
    importedAssetCount,
    parseStatusEntries: parseEntries,
    sourceLines: [
      datasetIdPresent ? '数据集：已选择' : '数据集：未选择',
      assetLibraryIdPresent ? `资产库：${assetLibraryName || '已选择'}` : '资产库：未选择',
      `资产数量：${importedAssetCount || requestedAssetCount}`,
      parseEntries.length
        ? `解析状态：${parseEntries.map((entry) => `${entry.label} ${entry.count}`).join(' / ')}`
        : '解析状态：待开始',
    ],
    editInfo: {
      promptSummary: trimmedString(input.promptSummary || input.prompt_summary || '导入设计图资产，解析画像后用于图库筛选、问答和报表。'),
      dataSources: [
        datasetIdPresent ? 'selected_dataset' : 'missing_dataset',
        assetLibraryIdPresent ? 'selected_asset_library' : 'missing_asset_library',
      ],
      fieldLedger: [
        'promptSummary',
        'dataSources',
        'parseStatusSummary',
        'fieldLedger',
        'validationReceipt',
      ],
    },
    detail: {
      promptSummary: trimmedString(input.promptSummary || input.prompt_summary || '导入设计图资产，解析画像后用于图库筛选、问答和报表。'),
      dataSources: {
        datasetSelected: datasetIdPresent,
        assetLibrarySelected: assetLibraryIdPresent,
        rawValuesIncluded: false,
      },
      parseStatusSummary: parseEntries,
      fieldLedger: {
        rawAssetValuesAllowed: false,
        sourceUrlValuesAllowed: false,
        secretMaterialAllowed: false,
        recordCountsOnly: true,
      },
      validationReceipt: {
        noChatMessage: true,
        noUnrelatedArtifactInjection: true,
        selectedCardRefreshOnly: true,
        noGlobalPolling: true,
      },
    },
    behavior: {
      cardPersistsAfterCreation: true,
      selectedCardRefreshOnly: true,
      noGlobalPolling: true,
      noChatMessage: true,
      noUnrelatedArtifactInjection: true,
    },
  };
}

export function buildFashionDesignAssetImportShelfTask({ assetLibrary, scope } = {}) {
  const normalizedScope = normalizeAssetLibraryScope(scope);
  const assetLibraryId = trimmedString(assetLibrary?.id);
  const scopedAssetLibraryId = trimmedString(
    normalizedScope.assetLibrary?.id
    || normalizedScope.assetLibrary?.asset_library_id
    || normalizedScope.assetLibrary?.assetLibraryId,
  );
  const assetIds = normalizedScope.assets
    .map((asset) => trimmedString(asset?.id))
    .filter(Boolean)
    .sort();
  if (
    !assetLibraryId
    || !assetIds.length
    || (scopedAssetLibraryId && scopedAssetLibraryId !== assetLibraryId)
  ) {
    return null;
  }

  const card = normalizeFashionDesignAssetImportTaskCardDraft({
    importId: `scope-${stableTaskDigest([assetLibraryId, ...assetIds])}`,
    assetLibraryId,
    assetLibraryName: assetLibrary.name,
    datasetId: normalizedScope.datasetIds[0] || '',
    requestedAssetCount: normalizedScope.assetCount,
    batchResponse: {
      accepted: true,
      asset_count: normalizedScope.assetCount,
      items: [],
    },
    scope: normalizedScope,
  });
  const updatedAt = normalizedScope.assets
    .map((asset) => trimmedString(asset?.updatedAt || asset?.createdAt))
    .filter(Boolean)
    .sort()
    .at(-1)
    || trimmedString(assetLibrary.updatedAt || assetLibrary.createdAt);
  const nextAction = card.status === 'failed'
    ? '检查失败状态后，在受控窗口内重试。'
    : card.status === 'completed'
      ? '资产画像已可用于图库筛选、问答和报表供料。'
      : '打开资产库查看最新解析状态。';

  return {
    id: card.id,
    title: card.title,
    capability: 'customer_artifact_request',
    route: 'asset_gallery_import_task',
    status: card.status,
    summary: card.subtitle,
    resultSummary: {
      summary: card.subtitle,
      findings: [
        '字段账本：仅展示提示、数据源、解析状态与验证回执。',
        ...card.sourceLines,
      ],
      recommendedNextActions: [nextAction],
      warnings: [],
    },
    createdAt: updatedAt,
    permissionScope: 'selected_dataset_and_asset_library',
    retryable: card.status === 'failed',
    assetImportTaskCard: card,
  };
}

export function normalizeAssetLibraryScope(payload) {
  const summary = payload?.summary || payload || {};
  const datasets = Array.isArray(summary.datasets) ? summary.datasets : [];
  const memberships = Array.isArray(summary.memberships) ? summary.memberships : [];
  const assets = Array.isArray(summary.assets) ? summary.assets : [];
  const normalizedAssets = assets.map(normalizeAssetItemSummary).filter(Boolean);
  const assetParseStatusCounts = normalizeCountMap(
    summary.asset_parse_status_counts || summary.assetParseStatusCounts,
  );
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
    assets: normalizedAssets,
    assetProfileHints: normalizedAssetProfileHints,
    datasetIds: normalizeStringIds(summary.dataset_ids || summary.datasetIds),
    deniedDatasetCount: Number(summary.denied_dataset_count ?? summary.deniedDatasetCount ?? 0) || 0,
    membershipCount: Number(summary.membership_count ?? summary.membershipCount ?? memberships.length) || 0,
    authorizedDatasetCount: Number(summary.authorized_dataset_count ?? summary.authorizedDatasetCount ?? datasets.length) || 0,
    assetCount: Number(summary.asset_count ?? summary.assetCount ?? normalizedAssets.length) || 0,
    assetProfileHintCount: Number(summary.asset_profile_hint_count ?? summary.assetProfileHintCount ?? normalizedAssetProfileHints.length) || 0,
    assetParseStatusCounts,
    assetParseRunCount: Number(summary.asset_parse_run_count ?? summary.assetParseRunCount ?? sumCountMap(assetParseStatusCounts)) || 0,
    scopePolicy: String(summary.scope_policy || summary.scopePolicy || '').trim(),
  };
}

function normalizeAssetItemSummary(asset) {
  if (!asset || typeof asset !== 'object') return null;
  const metadata = asset.metadata && typeof asset.metadata === 'object' && !Array.isArray(asset.metadata)
    ? asset.metadata
    : {};
  const id = trimmedString(asset.id || asset.asset_id || asset.assetId);
  const title = trimmedString(asset.title || asset.name || id);
  if (!id && !title) return null;
  return {
    id,
    assetLibraryId: trimmedString(asset.asset_library_id || asset.assetLibraryId),
    collectionId: trimmedString(asset.collection_id || asset.collectionId),
    externalId: trimmedString(asset.external_id || asset.externalId),
    title,
    assetKind: trimmedString(asset.asset_kind || asset.assetKind || 'asset') || 'asset',
    sourceKind: trimmedString(asset.source_kind || asset.sourceKind),
    contentType: trimmedString(asset.content_type || asset.contentType),
    profileCount: Number(asset.profile_count ?? asset.profileCount ?? 0) || 0,
    storageLocatorPresent: Boolean(trimmedString(asset.object_key || asset.objectKey))
      || metadata.storage_locator_present === true
      || metadata.storageLocatorPresent === true,
    sourceIdPresent: Boolean(trimmedString(asset.source_id || asset.sourceId))
      || metadata.source_id_present === true
      || metadata.sourceIdPresent === true,
    createdAt: trimmedString(asset.created_at || asset.createdAt),
    updatedAt: trimmedString(asset.updated_at || asset.updatedAt),
  };
}

function normalizeDatasetAssetMembershipSummary(membership) {
  if (!membership || typeof membership !== 'object') return null;
  const datasetId = trimmedString(membership.dataset_id || membership.datasetId);
  const assetId = trimmedString(membership.asset_id || membership.assetId);
  if (!datasetId && !assetId) return null;
  return {
    datasetId,
    assetId,
    membershipKind: trimmedString(membership.membership_kind || membership.membershipKind),
    expiresAt: trimmedString(membership.expires_at || membership.expiresAt),
    createdAt: trimmedString(membership.created_at || membership.createdAt),
  };
}

function normalizeAssetParseRunSummary(parseRun) {
  if (!parseRun || typeof parseRun !== 'object') return null;
  const id = trimmedString(parseRun.id || parseRun.parse_run_id || parseRun.parseRunId);
  const status = trimmedString(parseRun.status || parseRun.parse_status || parseRun.parseStatus);
  if (!id && !status) return null;
  return {
    id,
    assetId: trimmedString(parseRun.asset_id || parseRun.assetId),
    parserName: trimmedString(parseRun.parser_name || parseRun.parserName),
    parserVersion: trimmedString(parseRun.parser_version || parseRun.parserVersion),
    status,
    errorCode: trimmedString(parseRun.error_code || parseRun.errorCode),
    startedAt: trimmedString(parseRun.started_at || parseRun.startedAt),
    finishedAt: trimmedString(parseRun.finished_at || parseRun.finishedAt),
    createdAt: trimmedString(parseRun.created_at || parseRun.createdAt),
    updatedAt: trimmedString(parseRun.updated_at || parseRun.updatedAt),
  };
}

function normalizeAssetProfileViewSummary(profile) {
  if (!profile || typeof profile !== 'object') return null;
  const id = trimmedString(profile.id || profile.profile_id || profile.profileId);
  const profileKind = trimmedString(profile.profile_kind || profile.profileKind);
  if (!id && !profileKind) return null;
  const attributes = profile.attributes && typeof profile.attributes === 'object' && !Array.isArray(profile.attributes)
    ? profile.attributes
    : {};
  return {
    id,
    assetId: trimmedString(profile.asset_id || profile.assetId),
    profileKind,
    profileVersion: trimmedString(profile.profile_version || profile.profileVersion),
    embeddingStatus: trimmedString(profile.embedding_status || profile.embeddingStatus),
    attributesPresent: Object.keys(attributes).length > 0,
    createdAt: trimmedString(profile.created_at || profile.createdAt),
    updatedAt: trimmedString(profile.updated_at || profile.updatedAt),
  };
}

function normalizeCountMap(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return Object.fromEntries(
    Object.entries(value)
      .map(([key, count]) => [String(key || '').trim(), Number(count) || 0])
      .filter(([key, count]) => key && count > 0)
      .sort(([left], [right]) => left.localeCompare(right, 'zh-Hans-CN')),
  );
}

function sumCountMap(value) {
  return Object.values(value || {}).reduce((total, count) => total + (Number(count) || 0), 0);
}

const ASSET_PARSE_STATUS_LABELS = new Map([
  ['queued', '排队中'],
  ['pending', '待解析'],
  ['running', '解析中'],
  ['processing', '解析中'],
  ['retrying', '重试中'],
  ['completed', '已完成'],
  ['succeeded', '已完成'],
  ['success', '已完成'],
  ['failed', '失败'],
  ['error', '失败'],
  ['skipped', '已跳过'],
  ['unsupported', '暂不支持'],
]);

const ASSET_PARSE_STATUS_ORDER = new Map([
  ['running', 10],
  ['processing', 11],
  ['retrying', 12],
  ['queued', 20],
  ['pending', 21],
  ['failed', 30],
  ['error', 31],
  ['completed', 40],
  ['succeeded', 41],
  ['success', 42],
  ['skipped', 50],
  ['unsupported', 51],
]);

export function assetParseStatusEntries(scope, options = {}) {
  const limit = Number(options.limit || 6) || 6;
  return Object.entries(scope?.assetParseStatusCounts || {})
    .map(([status, count]) => {
      const normalizedStatus = String(status || '').trim();
      const value = Number(count) || 0;
      if (!normalizedStatus || value <= 0) return null;
      return {
        status: normalizedStatus,
        label: ASSET_PARSE_STATUS_LABELS.get(normalizedStatus) || normalizedStatus,
        count: value,
        order: ASSET_PARSE_STATUS_ORDER.get(normalizedStatus) ?? 100,
      };
    })
    .filter(Boolean)
    .sort((left, right) => (
      left.order - right.order
      || left.label.localeCompare(right.label, 'zh-Hans-CN')
      || left.status.localeCompare(right.status, 'zh-Hans-CN')
    ))
    .slice(0, Math.max(1, Math.min(limit, 20)))
    .map(({ order: _order, ...entry }) => entry);
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

function statusCountsFromBatchItems(items = []) {
  const counts = {};
  for (const item of items || []) {
    const status = trimmedString(item?.parseRunStatus || item?.status || 'pending') || 'pending';
    counts[status] = (counts[status] || 0) + 1;
  }
  return counts;
}

function normalizeGalleryTaskStatus(status) {
  const value = trimmedString(status).toLowerCase();
  if (['accepted', 'created', 'queued', 'pending'].includes(value)) return 'queued';
  if (['running', 'processing', 'parsing', 'in_progress'].includes(value)) return 'running';
  if (['retrying', 'retry'].includes(value)) return 'retrying';
  if (['completed', 'succeeded', 'success', 'done'].includes(value)) return 'completed';
  if (['failed', 'error'].includes(value)) return 'failed';
  if (['cancelled', 'canceled'].includes(value)) return 'cancelled';
  return 'queued';
}

function stableTaskToken(value) {
  return trimmedString(value)
    .replace(/[^a-zA-Z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 140)
    || 'draft';
}

function stableTaskDigest(values) {
  let left = 0x811c9dc5;
  let right = 0x9e3779b9;
  const input = (values || []).map((value) => trimmedString(value)).join('|');
  for (let index = 0; index < input.length; index += 1) {
    const code = input.charCodeAt(index);
    left = Math.imul(left ^ code, 0x01000193) >>> 0;
    right = Math.imul(right ^ (code + index), 0x85ebca6b) >>> 0;
  }
  return `${left.toString(16).padStart(8, '0')}${right.toString(16).padStart(8, '0')}`;
}

function normalizeTextList(values) {
  return [...new Set((Array.isArray(values) ? values : [])
    .map((value) => String(value || '').trim())
    .filter(Boolean))]
    .slice(0, 24);
}

function trimmedString(value) {
  return String(value || '').trim();
}

function parseJsonObjectDraft(value, textValue, label) {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return { value };
  }
  const text = trimmedString(textValue);
  if (!text) {
    return { value: {} };
  }
  try {
    const parsed = JSON.parse(text);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return { value: {}, error: `${label} 必须是 JSON 对象。` };
    }
    return { value: parsed };
  } catch {
    return { value: {}, error: `${label} 格式不正确。` };
  }
}

function sanitizeAssetImportMetadata(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return {};
  }
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => !assetImportMetadataKeyIsSensitive(key))
      .map(([key, item]) => [key, sanitizeAssetImportMetadataValue(item)]),
  );
}

function sanitizeAssetImportMetadataValue(value) {
  if (Array.isArray(value)) {
    return value.map(sanitizeAssetImportMetadataValue);
  }
  if (value && typeof value === 'object') {
    return sanitizeAssetImportMetadata(value);
  }
  return value;
}

function assetImportMetadataKeyIsSensitive(key) {
  return new Set([
    'source',
    'url',
    'image_url',
    'imageurl',
    'object_key',
    'objectkey',
    'package_source',
    'packagesource',
    'source_id',
    'sourceid',
    'filename',
    'file_name',
    'original_name',
    'path',
    'file_path',
    'local_path',
    'content_hash',
    'sha256',
    'raw_provider_payload',
    'provider_payload',
    'raw_payload',
    'authorization',
    'cookie',
  ]).has(String(key || '').trim().toLowerCase());
}

function inferAssetTitleFromSource(source) {
  const value = trimmedString(source);
  if (!value) return '';
  try {
    const url = new URL(value);
    const tail = url.pathname.split('/').filter(Boolean).pop();
    return decodeURIComponent(tail || url.hostname || value);
  } catch {
    return value.split(/[\\/]/).filter(Boolean).pop() || value;
  }
}

function inferImageContentType(source) {
  const lower = trimmedString(source).toLowerCase().split('?')[0];
  if (lower.endsWith('.png')) return 'image/png';
  if (lower.endsWith('.jpg') || lower.endsWith('.jpeg')) return 'image/jpeg';
  if (lower.endsWith('.webp')) return 'image/webp';
  if (lower.endsWith('.gif')) return 'image/gif';
  return '';
}

function parseAssetImportSources(value) {
  return [...new Set(String(value || '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean))]
    .slice(0, 100);
}

function normalizeAssetImportPackages(packages) {
  if (!Array.isArray(packages)) return [];
  return packages
    .map((item, index) => {
      const objectKey = trimmedString(item?.objectKey || item?.object_key || item?.url || item);
      if (!objectKey) return null;
      const title = trimmedString(item?.title || inferAssetTitleFromSource(objectKey));
      const externalId = trimmedString(item?.externalId || item?.external_id);
      const contentType = trimmedString(item?.contentType || item?.content_type) || 'application/zip';
      const metadata = item?.metadata && typeof item.metadata === 'object' && !Array.isArray(item.metadata)
        ? sanitizeAssetImportMetadata(item.metadata)
        : {};
      return {
        ...(externalId ? { external_id: externalId } : {}),
        ...(title ? { title } : {}),
        object_key: objectKey,
        content_type: contentType,
        metadata: {
          import_index: index,
          package_source_present: true,
          ...metadata,
        },
      };
    })
    .filter(Boolean)
    .slice(0, 20);
}

function looksLikeUrl(value) {
  return /^https?:\/\//i.test(trimmedString(value));
}
