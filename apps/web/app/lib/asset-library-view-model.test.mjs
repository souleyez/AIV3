import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  applyAssetLibraryPresetToDraft,
  assetLibraryContainsDataset,
  assetParseStatusEntries,
  assetProfileKindOptions,
  buildAssetLibraryCreatePayload,
  buildFashionDesignAssetImportShelfTask,
  buildFashionDesignImageAssetImportBatchPayload,
  buildFashionDesignImageAssetImportPayload,
  filterAssetProfileHints,
  normalizeFashionDesignAssetImportTaskCardDraft,
  normalizeFashionDesignImageAssetImportBatchResponse,
  normalizeFashionDesignImageAssetImportResponse,
  normalizeAssetProfileHint,
  normalizeAssetLibraries,
  normalizeAssetImportReadiness,
  normalizeAssetLibraryScope,
  selectedAssetLibraryView,
} from './asset-library-view-model.js';

describe('asset library view model', () => {
  it('keeps asset import disabled until authenticated readiness explicitly enables it', () => {
    assert.deepEqual(normalizeAssetImportReadiness(null), {
      enabled: false,
      reasonCode: 'not_ready',
      singleSupported: false,
      batchSupported: false,
      zipSupported: false,
    });
    assert.deepEqual(normalizeAssetImportReadiness({
      enabled: true,
      reason_code: 'enabled',
      single_supported: true,
      batch_supported: true,
      zip_supported: true,
      ignored_tenant: 'must-not-surface',
    }), {
      enabled: true,
      reasonCode: 'enabled',
      singleSupported: true,
      batchSupported: true,
      zipSupported: true,
    });
  });

  it('normalizes asset library list responses and sorts by recent update', () => {
    const libraries = normalizeAssetLibraries({
      asset_libraries: [
        { id: 'old', name: '旧资产库', domain: '', dataset_count: 1, updated_at: '2026-01-01T00:00:00Z' },
        { id: 'new', external_id: 'fashion-main', name: '服装设计资产库', domain: 'fashion_design', dataset_count: 3, updated_at: '2026-02-01T00:00:00Z' },
        { id: '', name: 'invalid' },
      ],
    });

    assert.equal(libraries.length, 2);
    assert.equal(libraries[0].id, 'new');
    assert.equal(libraries[0].externalId, 'fashion-main');
    assert.equal(libraries[0].datasetCount, 3);
    assert.equal(libraries[1].domain, 'general');
  });

  it('normalizes scope summaries without exposing denied dataset ids', () => {
    const scope = normalizeAssetLibraryScope({
      summary: {
        datasets: [{ id: 'dataset-visible' }],
        memberships: [{ dataset_id: 'dataset-visible' }],
        dataset_ids: ['dataset-visible'],
        assets: [{
          id: 'asset-visible',
          title: '资产图',
          asset_kind: 'image',
          source_kind: 'fashion_design_image_import',
          source_id: 'objects/private/look-001.png',
          object_key: 'objects/private/look-001.png',
          metadata: { raw_provider_payload: { should_not_surface: true } },
          profile_count: 1,
        }],
        asset_profile_hints: [{ asset_id: 'asset-visible', summary: '品类: 连衣裙' }],
        denied_dataset_count: 2,
        membership_count: 3,
        authorized_dataset_count: 1,
        asset_count: 1,
        asset_profile_hint_count: 1,
        asset_parse_status_counts: {
          pending: 2,
          completed: 1,
          ignored_zero: 0,
        },
        asset_parse_run_count: 3,
      },
    });

    assert.deepEqual(scope.datasetIds, ['dataset-visible']);
    assert.equal(scope.assets.length, 1);
    assert.equal(scope.assets[0].id, 'asset-visible');
    assert.equal(scope.assets[0].assetKind, 'image');
    assert.equal(scope.assets[0].profileCount, 1);
    assert.equal(scope.assets[0].storageLocatorPresent, true);
    assert.equal(scope.assets[0].sourceIdPresent, true);
    assert.equal(scope.assets[0].object_key, undefined);
    assert.equal(scope.assets[0].objectKey, undefined);
    assert.equal(scope.assets[0].source_id, undefined);
    assert.equal(scope.assets[0].sourceId, undefined);
    assert.equal(scope.assets[0].metadata, undefined);
    assert.equal(scope.assetProfileHints.length, 1);
    assert.equal(scope.deniedDatasetCount, 2);
    assert.equal(scope.membershipCount, 3);
    assert.equal(scope.authorizedDatasetCount, 1);
    assert.equal(scope.assetCount, 1);
    assert.equal(scope.assetProfileHintCount, 1);
    assert.deepEqual(scope.assetParseStatusCounts, { completed: 1, pending: 2 });
    assert.equal(scope.assetParseRunCount, 3);
    assert.equal(scope.assetProfileHints[0].assetId, 'asset-visible');
    assert.deepEqual(scope.assetProfileHints[0].nounTerms, []);
    assert.equal(assetLibraryContainsDataset(scope, 'dataset-visible'), true);
    assert.equal(assetLibraryContainsDataset(scope, 'dataset-hidden'), false);
  });

  it('normalizes asset parse status counts from camel-case payloads', () => {
    const scope = normalizeAssetLibraryScope({
      assetParseStatusCounts: {
        failed: 1,
        running: 2,
      },
    });

    assert.deepEqual(scope.assetParseStatusCounts, { failed: 1, running: 2 });
    assert.equal(scope.assetParseRunCount, 3);
  });

  it('preserves backend-redacted asset locator flags without raw asset fields', () => {
    const scope = normalizeAssetLibraryScope({
      assets: [{
        id: 'asset-redacted',
        title: '已脱敏素材',
        asset_kind: 'image',
        metadata: {
          storage_locator_present: true,
          source_id_present: true,
          metadata_present: true,
        },
      }],
    });

    assert.equal(scope.assets.length, 1);
    assert.equal(scope.assets[0].storageLocatorPresent, true);
    assert.equal(scope.assets[0].sourceIdPresent, true);
    assert.equal(scope.assets[0].object_key, undefined);
    assert.equal(scope.assets[0].source_id, undefined);
    assert.equal(scope.assets[0].metadata, undefined);
  });

  it('builds stable asset parse status entries for the main-site card', () => {
    const scope = normalizeAssetLibraryScope({
      asset_parse_status_counts: {
        completed: 5,
        pending: 3,
        running: 1,
        failed: 2,
        ignored_zero: 0,
      },
    });

    assert.deepEqual(assetParseStatusEntries(scope), [
      { status: 'running', label: '解析中', count: 1 },
      { status: 'pending', label: '待解析', count: 3 },
      { status: 'failed', label: '失败', count: 2 },
      { status: 'completed', label: '已完成', count: 5 },
    ]);
  });

  it('filters asset profile hints by asset kind and searchable terms', () => {
    const scope = normalizeAssetLibraryScope({
      asset_profile_hints: [
        {
          asset_id: 'asset-dress',
          title: '蓝色连衣裙',
          asset_kind: 'image',
          profile_kind: 'image_semantic',
          summary: '夏季通勤连衣裙主视觉',
          noun_terms: ['连衣裙', '蓝色'],
          facets: ['品类: 连衣裙'],
        },
        {
          asset_id: 'asset-video',
          title: '陈列视频',
          asset_kind: 'video',
          profile_kind: 'video_summary',
          summary: '门店陈列讲解',
          noun_terms: ['门店', '陈列'],
        },
      ],
    });

    assert.deepEqual(assetProfileKindOptions(scope), ['image', 'video']);
    assert.equal(filterAssetProfileHints(scope, { assetKind: 'image' }).length, 1);
    assert.equal(filterAssetProfileHints(scope, { query: '陈列' })[0].assetId, 'asset-video');
    assert.equal(filterAssetProfileHints(scope, { query: '不存在' }).length, 0);
  });

  it('drops empty asset profile hints and normalizes aliases', () => {
    assert.equal(normalizeAssetProfileHint({}), null);
    const hint = normalizeAssetProfileHint({
      assetId: 'asset-1',
      assetKind: 'presentation',
      profileKind: 'presentation_outline',
      nounTerms: ['经营分析', '经营分析'],
      facets: ['幻灯片数: 8'],
    });

    assert.equal(hint.assetId, 'asset-1');
    assert.equal(hint.assetKind, 'presentation');
    assert.deepEqual(hint.nounTerms, ['经营分析']);
    assert.ok(hint.searchText.includes('presentation_outline'));
  });

  it('selects the active asset library by id', () => {
    const libraries = normalizeAssetLibraries([
      { id: 'library-a', name: 'A' },
      { id: 'library-b', name: 'B' },
    ]);

    assert.equal(selectedAssetLibraryView(libraries, 'library-b')?.name, 'B');
    assert.equal(selectedAssetLibraryView(libraries, 'missing'), null);
  });

  it('applies sample presets without hardcoding domain behavior into core payloads', () => {
    const draft = applyAssetLibraryPresetToDraft({ name: '自定义资产库', domain: 'general' }, 'fashion_design');
    const payload = buildAssetLibraryCreatePayload(draft);

    assert.equal(payload.name, '服装设计资产库');
    assert.equal(payload.domain, 'fashion_design');
    assert.equal(payload.visibility, 'private');
    assert.equal(payload.metadata.created_from, 'main_workspace');
    assert.equal(payload.metadata.preset_id, 'fashion_design');
    assert.deepEqual(payload.metadata.recommended_collections, [
      '灵感图库',
      '设计稿',
      '款式文档',
      '供应链资料',
      '营销产物',
      '报表产物',
    ]);
  });

  it('builds a dataset-scoped fashion design image asset import payload', () => {
    const { payload, errors } = buildFashionDesignImageAssetImportPayload(
      {
        imageUrl: 'https://example.com/assets/dress-001.png',
        externalId: 'dress-001',
        profilePayloadText: '{"category":"dress","color":["green"]}',
        metadata: {
          operator: 'main-site',
          source: 'https://example.com/raw-source.png',
          original_name: '客户款式图.png',
          content_hash: 'abc123',
          raw_provider_payload: { should_not_surface: true },
          nested: {
            object_key: 'objects/private/look-001.png',
            local_path: 'C:/tmp/look-001.png',
            label: 'safe-label',
          },
        },
      },
      {
        datasetId: 'dataset-1',
        assetLibraryId: 'library-1',
      },
    );

    assert.deepEqual(errors, []);
    assert.equal(payload.dataset_id, 'dataset-1');
    assert.equal(payload.asset_library_id, 'library-1');
    assert.equal(payload.external_id, 'dress-001');
    assert.equal(payload.title, 'dress-001.png');
    assert.equal(payload.image_url, 'https://example.com/assets/dress-001.png');
    assert.equal(payload.content_type, 'image/png');
    assert.deepEqual(payload.profile_payload, { category: 'dress', color: ['green'] });
    assert.equal(payload.metadata.imported_from, 'main_workspace_asset_library');
    assert.equal(payload.metadata.operator, 'main-site');
    assert.equal(payload.metadata.source, undefined);
    assert.equal(payload.metadata.original_name, undefined);
    assert.equal(payload.metadata.content_hash, undefined);
    assert.equal(payload.metadata.raw_provider_payload, undefined);
    assert.equal(payload.metadata.nested.label, 'safe-label');
    assert.equal(payload.metadata.nested.object_key, undefined);
    assert.equal(payload.metadata.nested.local_path, undefined);
  });

  it('keeps fashion design image imports partial but rejects missing source and bad JSON', () => {
    const { payload, errors } = buildFashionDesignImageAssetImportPayload({
      datasetId: '',
      title: '',
      profilePayloadText: 'not-json',
    });

    assert.equal(payload.profile_payload.constructor, Object);
    assert.ok(errors.includes('请选择要写入的目标数据集。'));
    assert.ok(errors.includes('请输入图片 URL、对象 key 或外部 ID。'));
    assert.ok(errors.includes('画像 JSON 格式不正确。'));
  });

  it('requires an asset library before sending a collection-scoped image import', () => {
    const { errors } = buildFashionDesignImageAssetImportPayload({
      datasetId: 'dataset-1',
      collectionId: 'collection-1',
      imageUrl: 'https://example.com/assets/dress-001.png',
    });

    assert.ok(errors.includes('请选择资产库后再选择图库分组。'));
  });

  it('normalizes fashion design image asset import responses', () => {
    const normalized = normalizeFashionDesignImageAssetImportResponse({
      asset: {
        id: 'asset-1',
        title: '灵感图',
        asset_kind: 'image',
        source_kind: 'fashion_design_image_import',
        source_id: 'objects/private/look-001.png',
        object_key: 'objects/private/look-001.png',
        metadata: { raw_provider_payload: { should_not_surface: true } },
      },
      dataset_membership: { dataset_id: 'dataset-1', asset_id: 'asset-1' },
      parse_run: {
        id: 'parse-1',
        asset_id: 'asset-1',
        status: 'pending',
        parser_name: 'datamax-fashion-image-parser',
        error_message: 'private object path objects/private/look-001.png',
        metadata: { source: 'objects/private/look-001.png' },
      },
      profile: {
        id: 'profile-1',
        asset_id: 'asset-1',
        profile_kind: 'fashion_design_image_v1',
        attributes: {
          raw_provider_payload: { should_not_surface: true },
          category: 'dress',
        },
      },
    });

    assert.equal(normalized.assetId, 'asset-1');
    assert.equal(normalized.title, '灵感图');
    assert.equal(normalized.datasetId, 'dataset-1');
    assert.equal(normalized.parseRunStatus, 'pending');
    assert.equal(normalized.profileKind, 'fashion_design_image_v1');
    assert.equal(normalized.asset.storageLocatorPresent, true);
    assert.equal(normalized.asset.sourceIdPresent, true);
    assert.equal(normalized.datasetMembership.assetId, 'asset-1');
    assert.equal(normalized.parseRun.parserName, 'datamax-fashion-image-parser');
    assert.equal(normalized.parseRun.metadata, undefined);
    assert.equal(normalized.parseRun.error_message, undefined);
    assert.equal(normalized.profile.attributesPresent, true);
    assert.equal(normalized.profile.attributes, undefined);

    const serialized = JSON.stringify(normalized);
    assert.equal(serialized.includes('objects/private'), false);
    assert.equal(serialized.includes('raw_provider_payload'), false);
    assert.equal(serialized.includes('should_not_surface'), false);
  });

  it('builds a batch fashion design image asset import payload from pasted URLs', () => {
    const { payload, errors, assetCount } = buildFashionDesignImageAssetImportBatchPayload(
      {
        title: '春夏灵感',
        imageUrl: 'https://example.com/assets/dress-001.png\nassets/fashion/jacket-002.jpg',
        profilePayloadText: '{"style":["commute"]}',
        metadata: {
          upload_source: 'main_workspace_file_picker',
          url: 'https://example.com/raw-batch-url.png',
          file_name: '客户批量图.png',
          sha256: 'abc123',
          authorization: 'Bearer should-not-surface',
        },
      },
      {
        datasetId: 'dataset-1',
        assetLibraryId: 'library-1',
      },
    );

    assert.deepEqual(errors, []);
    assert.equal(assetCount, 2);
    assert.equal(payload.dataset_id, 'dataset-1');
    assert.equal(payload.asset_library_id, 'library-1');
    assert.equal(payload.assets[0].title, '春夏灵感 1');
    assert.equal(payload.assets[0].image_url, 'https://example.com/assets/dress-001.png');
    assert.equal(payload.assets[1].title, '春夏灵感 2');
    assert.equal(payload.assets[1].object_key, 'assets/fashion/jacket-002.jpg');
    assert.deepEqual(payload.assets[0].profile_payload, { style: ['commute'] });
    assert.equal(payload.assets[0].metadata.source, undefined);
    assert.equal(payload.assets[0].metadata.source_present, true);
    assert.equal(payload.assets[0].metadata.source_kind, 'image_url');
    assert.equal(payload.assets[1].metadata.source, undefined);
    assert.equal(payload.assets[1].metadata.source_kind, 'object_key');
    assert.equal(payload.metadata.import_mode, 'batch');
    assert.equal(payload.metadata.upload_source, 'main_workspace_file_picker');
    assert.equal(payload.metadata.url, undefined);
    assert.equal(payload.metadata.file_name, undefined);
    assert.equal(payload.metadata.sha256, undefined);
    assert.equal(payload.metadata.authorization, undefined);
  });

  it('requires an asset library before sending collection-scoped batch imports', () => {
    const { errors } = buildFashionDesignImageAssetImportBatchPayload({
      datasetId: 'dataset-1',
      collectionId: 'collection-1',
      imageUrl: 'https://example.com/assets/dress-001.png',
    });

    assert.ok(errors.includes('请选择资产库后再选择图库分组。'));
  });

  it('builds a batch fashion design image asset import payload from local object keys', () => {
    const { payload, errors, assetCount } = buildFashionDesignImageAssetImportBatchPayload(
      {
        imageUrl: 'local-uploads/2026/design-a.webp\nC:/tmp/design-b.png',
        metadata: { upload_source: 'main_workspace_file_picker' },
      },
      {
        datasetId: 'dataset-1',
        assetLibraryId: 'library-1',
      },
    );

    assert.deepEqual(errors, []);
    assert.equal(assetCount, 2);
    assert.equal(payload.assets[0].object_key, 'local-uploads/2026/design-a.webp');
    assert.equal(payload.assets[0].content_type, 'image/webp');
    assert.equal(payload.assets[1].object_key, 'C:/tmp/design-b.png');
    assert.equal(payload.assets[1].content_type, 'image/png');
    assert.equal(payload.assets[0].metadata.source, undefined);
    assert.equal(payload.assets[0].metadata.source_kind, 'object_key');
    assert.equal(payload.assets[1].metadata.source, undefined);
    assert.equal(payload.assets[1].metadata.source_kind, 'object_key');
    assert.equal(payload.metadata.upload_source, 'main_workspace_file_picker');
  });

  it('builds a batch fashion design image asset import payload from zip packages', () => {
    const { payload, errors, assetCount } = buildFashionDesignImageAssetImportBatchPayload(
      {
        packages: [
          {
            title: '夏季图库',
            object_key: 'C:/tmp/fashion-assets.zip',
            content_type: 'application/zip',
            metadata: {
              upload_source: 'main_workspace_file_picker',
              original_name: '客户图库.zip',
              path: 'C:/tmp/fashion-assets.zip',
              object_key: 'C:/tmp/fashion-assets.zip',
              raw_provider_payload: { should_not_surface: true },
            },
          },
        ],
      },
      {
        datasetId: 'dataset-1',
        assetLibraryId: 'library-1',
      },
    );

    assert.deepEqual(errors, []);
    assert.equal(assetCount, 0);
    assert.equal(payload.assets.length, 0);
    assert.equal(payload.packages.length, 1);
    assert.equal(payload.packages[0].object_key, 'C:/tmp/fashion-assets.zip');
    assert.equal(payload.packages[0].title, '夏季图库');
    assert.equal(payload.packages[0].metadata.upload_source, 'main_workspace_file_picker');
    assert.equal(payload.packages[0].metadata.package_source, undefined);
    assert.equal(payload.packages[0].metadata.package_source_present, true);
    assert.equal(payload.packages[0].metadata.original_name, undefined);
    assert.equal(payload.packages[0].metadata.path, undefined);
    assert.equal(payload.packages[0].metadata.object_key, undefined);
    assert.equal(payload.packages[0].metadata.raw_provider_payload, undefined);
  });

  it('normalizes batch fashion design image asset import responses', () => {
    const normalized = normalizeFashionDesignImageAssetImportBatchResponse({
      accepted: true,
      asset_count: 2,
      package_count: 1,
      expanded_asset_count: 2,
      items: [
        { asset: { id: 'asset-1', title: '图 1' } },
        { asset: { id: 'asset-2', title: '图 2' } },
      ],
    });

    assert.equal(normalized.accepted, true);
    assert.equal(normalized.assetCount, 2);
    assert.equal(normalized.packageCount, 1);
    assert.equal(normalized.expandedAssetCount, 2);
    assert.deepEqual(normalized.items.map((item) => item.assetId), ['asset-1', 'asset-2']);
  });

  it('builds stable main-site gallery task cards for asset imports without raw source leakage', () => {
    const card = normalizeFashionDesignAssetImportTaskCardDraft({
      importId: 'import-001',
      assetLibraryId: 'library-1',
      assetLibraryName: '服装设计资产库',
      datasetId: 'dataset-1',
      promptSummary: '导入春夏款式图，按风格和颜色形成画像。',
      batchResponse: {
        accepted: true,
        asset_count: 2,
        items: [
          {
            asset: {
              id: 'asset-1',
              title: 'spring-dress.png',
              object_key: 'objects/private/spring-dress.png',
            },
            parse_run: { id: 'parse-1', status: 'pending' },
          },
          {
            asset: {
              id: 'asset-2',
              title: 'workwear.webp',
              source_id: 'https://example.com/workwear.webp',
            },
            parse_run: { id: 'parse-2', status: 'completed' },
          },
        ],
      },
      scope: {
        summary: {
          asset_parse_status_counts: {
            pending: 1,
            completed: 1,
          },
        },
      },
    });

    assert.equal(card.id, 'asset-gallery-import:import-001');
    assert.equal(card.type, 'asset_gallery_import_task');
    assert.equal(card.title, '图库任务：服装设计资产库');
    assert.equal(card.status, 'queued');
    assert.equal(card.datasetIdPresent, true);
    assert.equal(card.assetLibraryIdPresent, true);
    assert.equal(card.importedAssetCount, 2);
    assert.equal(card.behavior.cardPersistsAfterCreation, true);
    assert.equal(card.behavior.selectedCardRefreshOnly, true);
    assert.equal(card.behavior.noGlobalPolling, true);
    assert.equal(card.behavior.noChatMessage, true);
    assert.equal(card.behavior.noUnrelatedArtifactInjection, true);
    assert.equal(card.detail.dataSources.rawValuesIncluded, false);
    assert.equal(card.detail.fieldLedger.rawAssetValuesAllowed, false);
    assert.equal(card.detail.validationReceipt.noUnrelatedArtifactInjection, true);
    assert.ok(card.sourceLines.some((line) => line.includes('资产库：服装设计资产库')));
    assert.ok(card.editInfo.fieldLedger.includes('validationReceipt'));

    const serialized = JSON.stringify(card);
    assert.equal(serialized.includes('objects/private'), false);
    assert.equal(serialized.includes('https://example.com'), false);
    assert.equal(/object[_-]?key/i.test(serialized), false);
  });

  it('builds one persistent shelf task from asset-library scope and deduplicates reordered assets', () => {
    const assetLibrary = {
      id: 'library-private-001',
      name: '服装设计资产库',
      updatedAt: '2026-07-10T10:00:00Z',
    };
    const scope = {
      summary: {
        asset_library: { id: 'library-private-001', name: '服装设计资产库' },
        dataset_ids: ['dataset-private-001'],
        asset_count: 2,
        asset_parse_status_counts: { pending: 2 },
        asset_parse_run_count: 2,
        assets: [
          {
            id: 'asset-b',
            title: 'workwear.webp',
            object_key: 'objects/private/workwear.webp',
            updated_at: '2026-07-10T10:02:00Z',
          },
          {
            id: 'asset-a',
            title: 'spring-dress.png',
            source_id: 'https://example.com/spring-dress.png',
            updated_at: '2026-07-10T10:01:00Z',
          },
        ],
      },
    };

    const first = buildFashionDesignAssetImportShelfTask({ assetLibrary, scope });
    const reordered = buildFashionDesignAssetImportShelfTask({
      assetLibrary,
      scope: {
        summary: {
          ...scope.summary,
          assets: [...scope.summary.assets].reverse(),
        },
      },
    });

    assert.equal(first.id, reordered.id);
    assert.equal(first.capability, 'customer_artifact_request');
    assert.equal(first.route, 'asset_gallery_import_task');
    assert.equal(first.status, 'queued');
    assert.equal(first.resultSummary.findings.some((item) => item.startsWith('字段账本：')), true);
    assert.equal(first.assetImportTaskCard.behavior.cardPersistsAfterCreation, true);
    assert.equal(first.assetImportTaskCard.detail.dataSources.rawValuesIncluded, false);

    const serialized = JSON.stringify(first);
    assert.equal(serialized.includes('objects/private'), false);
    assert.equal(serialized.includes('https://example.com'), false);
    assert.equal(/object[_-]?key/i.test(serialized), false);
    assert.equal(buildFashionDesignAssetImportShelfTask({ assetLibrary, scope: { summary: {} } }), null);
    assert.equal(buildFashionDesignAssetImportShelfTask({
      assetLibrary,
      scope: {
        summary: {
          ...scope.summary,
          asset_library: { id: 'another-library' },
        },
      },
    }), null);
  });
});
