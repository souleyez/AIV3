import test from 'node:test';
import assert from 'node:assert/strict';
import {
  applyStaticPageOperation,
  buildInitialStaticPageDraft,
  buildStaticPageFinalRenderPayload,
  buildStaticPageImagePayload,
  validateMobileOrder,
  validateStaticPageLayout,
} from './static-page-draft.js';

test('buildInitialStaticPageDraft creates default modules and mobile order', () => {
  const draft = buildInitialStaticPageDraft({
    datasetId: 'dataset-1',
    sessionId: 'session-1',
    conversationSummary: '客户需要一页经营摘要。',
    evidenceIds: ['ev-1'],
  });

  assert.equal(draft.datasetId, 'dataset-1');
  assert.equal(draft.sessionId, 'session-1');
  assert.equal(draft.modules.length, 5);
  assert.deepEqual(draft.mobileOrder, draft.modules.map((module) => module.id));
  assert.equal(draft.source.evidenceIds[0], 'ev-1');
});

test('applyStaticPageOperation updates module copy without mutating original draft', () => {
  const draft = buildInitialStaticPageDraft();
  const next = applyStaticPageOperation(draft, {
    type: 'update_module',
    targetModuleId: 'hero',
    patch: {
      title: '董事会核心判断',
      content: '增长放缓，但高价值客户贡献提升。',
    },
  });

  assert.equal(draft.modules[0].title, '核心判断');
  assert.equal(next.modules[0].title, '董事会核心判断');
  assert.equal(next.modules[0].content, '增长放缓，但高价值客户贡献提升。');
});

test('desktop layout resize is clamped inside the 12 column grid', () => {
  const draft = buildInitialStaticPageDraft();
  const next = applyStaticPageOperation(draft, {
    type: 'resize_module',
    targetModuleId: 'trend',
    layout: { x: 10, w: 8, h: 0 },
  });

  const layout = next.modules.find((module) => module.id === 'trend').layout;
  assert.equal(layout.x + layout.w <= 12, true);
  assert.equal(layout.h, 1);
  assert.equal(validateStaticPageLayout(layout), true);
});

test('mobile reorder accepts only known modules and fills missing modules', () => {
  const draft = buildInitialStaticPageDraft();
  const next = applyStaticPageOperation(draft, {
    type: 'reorder_modules',
    order: ['risk', 'hero', 'unknown', 'risk'],
  });

  assert.equal(next.mobileOrder[0], 'risk');
  assert.equal(next.mobileOrder[1], 'hero');
  assert.equal(validateMobileOrder(next.modules, next.mobileOrder), true);
});

test('style direction and visualization operations are validated', () => {
  const draft = buildInitialStaticPageDraft();
  const withStyle = applyStaticPageOperation(draft, {
    type: 'change_style_direction',
    styleDirection: 'decision-brief',
  });
  const withChart = applyStaticPageOperation(withStyle, {
    type: 'change_visualization',
    targetModuleId: 'trend',
    visualizationType: 'bar-chart',
  });
  const invalidStyle = applyStaticPageOperation(withChart, {
    type: 'change_style_direction',
    styleDirection: 'unknown-style',
  });

  assert.equal(withChart.styleDirection, 'decision-brief');
  assert.equal(withChart.modules.find((module) => module.id === 'trend').visualization.type, 'bar-chart');
  assert.equal(invalidStyle.styleDirection, 'decision-brief');
});

test('image and final render payloads include confirmed structure', () => {
  const draft = applyStaticPageOperation(buildInitialStaticPageDraft({ datasetId: 'dataset-1' }), {
    type: 'confirm_preview',
    previewImage: { assetKey: 'preview-1.png' },
  });

  const imagePayload = buildStaticPageImagePayload(draft, { oneClick: true });
  const finalPayload = buildStaticPageFinalRenderPayload(draft);

  assert.equal(imagePayload.oneClick, true);
  assert.equal(imagePayload.modules.length, 5);
  assert.equal(finalPayload.previewImage.assetKey, 'preview-1.png');
  assert.equal(validateMobileOrder(finalPayload.modules, finalPayload.mobileOrder), true);
});
