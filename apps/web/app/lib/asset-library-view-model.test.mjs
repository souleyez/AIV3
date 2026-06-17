import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  applyAssetLibraryPresetToDraft,
  assetLibraryContainsDataset,
  assetProfileKindOptions,
  buildAssetLibraryCreatePayload,
  filterAssetProfileHints,
  normalizeAssetProfileHint,
  normalizeAssetLibraries,
  normalizeAssetLibraryScope,
  selectedAssetLibraryView,
} from './asset-library-view-model.js';

describe('asset library view model', () => {
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
        assets: [{ id: 'asset-visible', title: '资产图' }],
        asset_profile_hints: [{ asset_id: 'asset-visible', summary: '品类: 连衣裙' }],
        denied_dataset_count: 2,
        membership_count: 3,
        authorized_dataset_count: 1,
        asset_count: 1,
        asset_profile_hint_count: 1,
      },
    });

    assert.deepEqual(scope.datasetIds, ['dataset-visible']);
    assert.equal(scope.assets.length, 1);
    assert.equal(scope.assetProfileHints.length, 1);
    assert.equal(scope.deniedDatasetCount, 2);
    assert.equal(scope.membershipCount, 3);
    assert.equal(scope.authorizedDatasetCount, 1);
    assert.equal(scope.assetCount, 1);
    assert.equal(scope.assetProfileHintCount, 1);
    assert.equal(scope.assetProfileHints[0].assetId, 'asset-visible');
    assert.deepEqual(scope.assetProfileHints[0].nounTerms, []);
    assert.equal(assetLibraryContainsDataset(scope, 'dataset-visible'), true);
    assert.equal(assetLibraryContainsDataset(scope, 'dataset-hidden'), false);
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
});
