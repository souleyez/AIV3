import test from 'node:test';
import assert from 'node:assert/strict';
import {
  applyStaticPageOperation,
  applyStaticPageOperations,
  buildInitialStaticPageDraft,
  buildMockStaticPagePreview,
  buildStaticPageFinalRenderPayload,
  buildStaticPageImagePayload,
  interpretStaticPagePrompt,
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

test('prompt interpreter changes tone for decision makers', () => {
  const draft = buildInitialStaticPageDraft();
  const interpretation = interpretStaticPagePrompt(draft, '整体更像给老板看的，结论先行');
  const next = applyStaticPageOperations(draft, interpretation.operations);

  assert.equal(next.styleDirection, 'decision-brief');
  assert.match(next.modelSummary, /高层决策简报/);
});

test('prompt interpreter highlights risk and moves it first on mobile', () => {
  const draft = buildInitialStaticPageDraft();
  const interpretation = interpretStaticPagePrompt(draft, '突出风险，把风险放到前面');
  const next = applyStaticPageOperations(draft, interpretation.operations);

  assert.equal(next.mobileOrder[0], 'risk');
  assert.equal(next.modules.find((module) => module.id === 'risk').title, '优先风险与机会');
  assert.equal(validateMobileOrder(next.modules, next.mobileOrder), true);
});

test('prompt interpreter switches a target module chart type', () => {
  const draft = buildInitialStaticPageDraft();
  const interpretation = interpretStaticPagePrompt(draft, '把趋势变化换成柱状图');
  const next = applyStaticPageOperations(draft, interpretation.operations);

  assert.equal(next.modules.find((module) => module.id === 'trend').visualization.type, 'bar-chart');
});

test('prompt interpreter reduces copy across modules', () => {
  const draft = buildInitialStaticPageDraft();
  const interpretation = interpretStaticPagePrompt(draft, '减少文字，整体精简一点');
  const next = applyStaticPageOperations(draft, interpretation.operations);

  assert.equal(next.modules.every((module) => module.content.includes('减少解释性文字')), true);
  assert.match(next.modelSummary, /压缩所有模块文案/);
});

test('prompt interpreter adds an evidence data module', () => {
  const draft = buildInitialStaticPageDraft();
  const interpretation = interpretStaticPagePrompt(draft, '新增一个数据来源明细，最好用表格');
  const next = applyStaticPageOperations(draft, interpretation.operations);
  const addedModule = next.modules.find((module) => module.id === 'evidence-6');

  assert.equal(next.modules.length, 6);
  assert.equal(addedModule.visualization.type, 'table');
  assert.equal(validateMobileOrder(next.modules, next.mobileOrder), true);
});

test('prompt interpreter selects data dashboard direction', () => {
  const draft = buildInitialStaticPageDraft();
  const interpretation = interpretStaticPagePrompt(draft, '做成运营看板，提高数据密度');
  const next = applyStaticPageOperations(draft, interpretation.operations);

  assert.equal(next.styleDirection, 'data-command');
  assert.match(next.modelSummary, /数据运营看板/);
});

test('image queue flow creates deterministic mock preview and confirmation state', () => {
  const draft = buildInitialStaticPageDraft({ datasetId: 'dataset-1' });
  const queued = applyStaticPageOperation(draft, {
    type: 'queue_image_job',
    queuePosition: 3,
  });
  const ready = applyStaticPageOperation(queued, { type: 'mark_preview_ready' });
  const confirmed = applyStaticPageOperation(ready, { type: 'confirm_preview' });

  assert.equal(queued.status, 'queued');
  assert.equal(queued.imageJob.status, 'queued');
  assert.equal(queued.imageJob.queueMessage, '资源正在排队，可以联系商务开通高级用户跳过等待。');
  assert.equal(ready.status, 'preview_ready');
  assert.equal(ready.previewImage.kind, 'mock-effect-preview');
  assert.equal(ready.previewImage.modules.length, ready.modules.length);
  assert.equal(confirmed.status, 'effect_confirmed');
});

test('mock preview payload follows current style and module layout', () => {
  const draft = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'change_style_direction',
    styleDirection: 'data-command',
  });
  const preview = buildMockStaticPagePreview(draft);

  assert.equal(preview.title, '数据运营看板');
  assert.equal(preview.modules[0].width, 12);
});

test('final render request stores local mock renderer payload', () => {
  const confirmed = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'confirm_preview',
    previewImage: { assetKey: 'preview-1.png' },
  });
  const rendering = applyStaticPageOperation(confirmed, { type: 'request_final_render' });

  assert.equal(rendering.status, 'rendering');
  assert.equal(rendering.finalPage.status, 'mock_ready');
  assert.equal(rendering.finalPage.renderer, 'local-static-page-mock');
  assert.equal(rendering.finalPage.payload.previewImage.assetKey, 'preview-1.png');
});

test('final render request accepts backend rendered output', () => {
  const confirmed = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'confirm_preview',
    previewImage: { assetKey: 'preview-1.png' },
  });
  const rendered = applyStaticPageOperation(confirmed, {
    type: 'request_final_render',
    finalPage: {
      status: 'rendered',
      renderer: 'platform-api-static-page-renderer',
      renderOutputId: 'render-1',
      html: '<main>核心判断</main>',
      assetManifest: { preview_asset_key: 'preview-1.png' },
    },
  });

  assert.equal(rendered.status, 'rendered');
  assert.equal(rendered.finalPage.status, 'rendered');
  assert.equal(rendered.finalPage.renderOutputId, 'render-1');
  assert.match(rendered.finalPage.html, /核心判断/);
});
