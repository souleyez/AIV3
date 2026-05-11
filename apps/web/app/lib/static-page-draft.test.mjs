import test from 'node:test';
import assert from 'node:assert/strict';
import {
  applyStaticPageOperation,
  applyStaticPageOperations,
  buildInitialStaticPageDraft,
  buildMockStaticPagePreview,
  buildStaticPageDataSourceCandidates,
  buildStaticPageFieldCandidates,
  buildStaticPageFinalRenderPayload,
  buildStaticPageImagePayload,
  buildStaticPageModuleUpdateOperation,
  buildStaticPagePreviewContract,
  buildStaticPageRenderSpec,
  buildStaticPageVisualSpec,
  canRequestStaticPageFinalRender,
  interpretStaticPagePrompt,
  staticPageFinalRenderBlockReason,
  staticPagePreviewBlockReason,
  validateMobileOrder,
  validateStaticPageLayout,
} from './static-page-draft.js';
import {
  buildStaticPageEchartsPreviewOption,
  hasRenderableEchartsData,
  normalizeStaticPageChartRows,
  staticPageChartRowsFromModule,
} from './static-page-chart-runtime.js';

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
  assert.equal(draft.visualSpec.styleDirection, 'client-delivery');
  assert.equal(draft.renderSpec.componentModel, 'dom-text-svg-chart');
  assert.equal(draft.previewContract.status, 'not_requested');
  assert.equal(draft.dataSnapshot.moduleBindings.length, 5);
  assert.ok(draft.dataSnapshot.dataSourceCandidates.some((item) => item.sourceId === 'selected_scope'));
  assert.ok(draft.dataSnapshot.fieldCandidates.some((item) => item.fieldPath === 'retrieval.summary'));
  assert.equal(draft.dataSnapshot.moduleBindings.find((item) => item.moduleId === 'hero').bindingQualityStatus, 'confirmed');
  assert.equal(draft.dataSnapshot.moduleBindings.find((item) => item.moduleId === 'trend').chartDataFit, 'needs_sample_rows');
});

test('preview queue gate blocks chart modules without renderable sample rows', () => {
  const draft = buildInitialStaticPageDraft({
    datasetId: 'dataset-1',
    sessionId: 'session-1',
  });

  const blockReason = staticPagePreviewBlockReason(draft);
  assert.match(blockReason, /数据绑定未达到效果图生成要求/);
  assert.match(blockReason, /关键指标/);
  assert.match(blockReason, /needs_sample_rows/);

  const ready = applyStaticPageOperations(draft, [
    {
      type: 'update_module',
      targetModuleId: 'kpi',
      patch: {
        visualization: {
          type: 'kpi-cards',
          data: [
            { label: '收入', value: 1200 },
            { label: '客户数', value: 320 },
          ],
        },
      },
    },
    {
      type: 'update_module',
      targetModuleId: 'trend',
      patch: {
        visualization: {
          type: 'line-chart',
          data: [
            { label: '1月', value: 1200 },
            { label: '2月', value: 1380 },
          ],
        },
      },
    },
    {
      type: 'update_module',
      targetModuleId: 'risk',
      patch: {
        visualization: {
          type: 'risk-matrix',
          data: [
            { label: '交付风险', value: 0.7 },
            { label: '机会空间', value: 0.4 },
          ],
        },
      },
    },
  ]);

  assert.equal(staticPagePreviewBlockReason(ready), '');
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
  assert.equal(withChart.visualSpec.styleDirection, 'decision-brief');
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
  assert.equal(imagePayload.designContract.editableCore, 'dom-text-svg-chart');
  assert.equal(imagePayload.previewContract.status, 'confirmed');
  assert.equal(finalPayload.previewImage.assetKey, 'preview-1.png');
  assert.equal(finalPayload.visualSpec.styleDirection, 'client-delivery');
  assert.equal(validateMobileOrder(finalPayload.modules, finalPayload.mobileOrder), true);
});

test('design contract fingerprint changes after layout or content edits', () => {
  const draft = buildInitialStaticPageDraft();
  const originalContract = buildStaticPagePreviewContract(draft);
  const edited = applyStaticPageOperation(draft, {
    type: 'update_module',
    targetModuleId: 'hero',
    patch: { content: '更新后的核心判断。' },
  });

  assert.notEqual(edited.previewContract.draftFingerprint, originalContract.draftFingerprint);
  assert.equal(edited.previewContract.status, 'not_requested');
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

test('prompt-based static page edits invalidate queued preview work', () => {
  const queued = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'queue_image_job',
    jobId: 'job-prompt-queued',
    queuePosition: 2,
  });
  const interpretation = interpretStaticPagePrompt(queued, '把趋势变化换成柱状图');
  const next = applyStaticPageOperations(queued, interpretation.operations);

  assert.equal(next.modules.find((module) => module.id === 'trend').visualization.type, 'bar-chart');
  assert.equal(next.previewContract.status, 'stale');
  assert.equal(next.imageJob.id, 'job-prompt-queued');
  assert.equal(next.imageJob.status, 'stale');
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

test('requeueing an image job clears stale preview and final render state', () => {
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
      html: '<main>旧结果</main>',
    },
  });
  const queued = applyStaticPageOperation(rendered, {
    type: 'queue_image_job',
    jobId: 'job-2',
  });

  assert.equal(queued.status, 'queued');
  assert.equal(queued.imageJob.id, 'job-2');
  assert.equal(queued.previewImage, null);
  assert.equal(queued.finalPage, null);
});

test('resetting an image job clears preview and final render state', () => {
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
      html: '<main>旧结果</main>',
    },
  });
  const reset = applyStaticPageOperation(rendered, { type: 'reset_image_job' });

  assert.equal(reset.status, 'planning');
  assert.equal(reset.imageJob.status, 'idle');
  assert.equal(reset.previewImage, null);
  assert.equal(reset.finalPage, null);
  assert.equal(reset.previewContract.status, 'not_requested');
});

test('resetting final render keeps confirmed preview for stage rollback', () => {
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
      html: '<main>旧结果</main>',
    },
  });
  const reset = applyStaticPageOperation(rendered, { type: 'reset_final_render' });

  assert.equal(reset.status, 'effect_confirmed');
  assert.equal(reset.finalPage, null);
  assert.equal(reset.previewImage.assetKey, 'preview-1.png');
  assert.equal(reset.previewContract.status, 'confirmed');
});

test('failed image job status returns draft to editable planning state', () => {
  const queued = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'queue_image_job',
    jobId: 'job-1',
  });
  const failed = applyStaticPageOperation(queued, {
    type: 'update_image_job_status',
    status: 'failed',
    queueMessage: 'CODEX_AUTH_REQUIRED',
  });

  assert.equal(failed.status, 'planning');
  assert.equal(failed.imageJob.status, 'failed');
  assert.equal(failed.imageJob.queueMessage, 'CODEX_AUTH_REQUIRED');
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

test('final render can only start from current confirmed preview', () => {
  const confirmed = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'confirm_preview',
    previewImage: { assetKey: 'preview-1.png' },
  });
  const stale = applyStaticPageOperation(confirmed, {
    type: 'update_module',
    targetModuleId: 'hero',
    patch: { content: '确认效果图后又改了核心判断。' },
  });
  const missingAsset = {
    ...confirmed,
    previewImage: null,
    previewContract: {
      ...confirmed.previewContract,
      assetKey: null,
    },
  };
  const failedFinalRender = applyStaticPageOperation(confirmed, {
    type: 'request_final_render',
    finalPage: {
      status: 'failed',
      renderer: 'platform-api-static-page-renderer',
      assetManifest: {
        workflow: { executionId: 'workflow-final-failed' },
      },
    },
  });

  assert.equal(canRequestStaticPageFinalRender(confirmed), true);
  assert.equal(staticPageFinalRenderBlockReason(confirmed), '');
  assert.equal(canRequestStaticPageFinalRender(failedFinalRender), true);
  assert.equal(staticPageFinalRenderBlockReason(failedFinalRender), '');
  assert.equal(canRequestStaticPageFinalRender(stale), false);
  assert.match(staticPageFinalRenderBlockReason(stale), /重新生成并确认效果图/);
  assert.equal(canRequestStaticPageFinalRender(missingAsset), false);
  assert.match(staticPageFinalRenderBlockReason(missingAsset), /资源缺失/);
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

test('editing after preview confirmation marks the visual contract stale', () => {
  const confirmed = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'confirm_preview',
    previewImage: { assetKey: 'preview-1.png' },
  });
  const edited = applyStaticPageOperation(confirmed, {
    type: 'change_style_direction',
    styleDirection: 'data-command',
  });

  assert.equal(edited.previewContract.status, 'stale');
  assert.equal(edited.previewContract.previousAssetKey, 'preview-1.png');
  assert.equal(edited.previewImage, null);
  assert.equal(edited.imageJob.status, 'stale');
  assert.match(edited.imageJob.queueMessage, /重新生成/);
});

test('editing while image generation is queued marks the queued preview stale', () => {
  const queued = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'queue_image_job',
    jobId: 'job-queued-1',
    queuePosition: 4,
  });
  const edited = applyStaticPageOperation(queued, {
    type: 'update_module',
    targetModuleId: 'hero',
    patch: { content: '排队过程中用户更新了核心判断。' },
  });

  assert.equal(edited.previewContract.status, 'stale');
  assert.equal(edited.previewContract.imageJobId, 'job-queued-1');
  assert.equal(edited.imageJob.id, 'job-queued-1');
  assert.equal(edited.imageJob.status, 'stale');
  assert.equal(edited.imageJob.queuePosition, null);
  assert.equal(edited.previewImage, null);
  assert.equal(edited.finalPage, null);
});

test('editing after final render clears stale final page output', () => {
  const confirmed = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'confirm_preview',
    previewImage: { assetKey: 'preview-1.png' },
  });
  const rendered = applyStaticPageOperation(confirmed, {
    type: 'request_final_render',
    finalPage: {
      status: 'rendered',
      renderer: 'platform-api-static-page-renderer',
      html: '<main>旧页面</main>',
    },
  });
  const edited = applyStaticPageOperation(rendered, {
    type: 'update_module',
    targetModuleId: 'trend',
    patch: {
      visualization: {
        data: [{ label: '三月', value: 1510 }],
      },
    },
  });

  assert.equal(edited.previewContract.status, 'stale');
  assert.equal(edited.finalPage, null);
  assert.equal(edited.previewImage, null);
});

test('module edit patch can update copy data binding and chart as one durable operation', () => {
  const confirmed = applyStaticPageOperation(buildInitialStaticPageDraft(), {
    type: 'confirm_preview',
    previewImage: { assetKey: 'preview-1.png' },
  });
  const edited = applyStaticPageOperation(confirmed, {
    type: 'update_module',
    targetModuleId: 'trend',
    patch: {
      title: '客户增长趋势',
      content: '重点展示最近三个月高价值客户增长。',
      dataBinding: {
        label: '客户订单趋势数据',
      },
      visualization: {
        type: 'bar-chart',
        label: '分类对比柱状图',
      },
    },
  });
  const module = edited.modules.find((item) => item.id === 'trend');

  assert.equal(module.title, '客户增长趋势');
  assert.equal(module.content, '重点展示最近三个月高价值客户增长。');
  assert.equal(module.dataBinding.label, '客户订单趋势数据');
  assert.equal(module.visualization.type, 'bar-chart');
  assert.equal(edited.dataSnapshot.moduleBindings.find((item) => item.moduleId === 'trend').binding.label, '客户订单趋势数据');
  assert.equal(edited.previewContract.status, 'stale');
  assert.equal(edited.finalPage, null);
});

test('chart runtime defaults to deterministic and appears in snapshots and payloads', () => {
  const draft = buildInitialStaticPageDraft({ datasetId: 'dataset-1' });
  const trend = draft.modules.find((module) => module.id === 'trend');
  const binding = draft.dataSnapshot.moduleBindings.find((item) => item.moduleId === 'trend');
  const imagePayload = buildStaticPageImagePayload(draft);

  assert.equal(trend.visualization.chartRuntime, 'deterministic');
  assert.equal(binding.chartRuntime, 'deterministic');
  assert.equal(imagePayload.modules.find((module) => module.id === 'trend').chartRuntime, 'deterministic');
  assert.equal(draft.renderSpec.chartRuntime, 'deterministic-with-echarts-advanced');
});

test('module edit can request sanitized ECharts runtime for advanced charts', () => {
  const draft = buildInitialStaticPageDraft();
  const next = applyStaticPageOperation(draft, {
    type: 'update_module',
    targetModuleId: 'trend',
    patch: {
      visualization: {
        type: 'bar-chart',
        chartRuntime: 'echarts',
        chartOptions: {
          color: ['#2563eb', '#14b8a6'],
          xAxis: { type: 'category', data: ['一月', '二月'] },
          yAxis: { type: 'value' },
          tooltip: { trigger: 'axis' },
          series: [
            { type: 'bar', name: '订单', data: [12, 18] },
          ],
        },
      },
    },
  });
  const module = next.modules.find((item) => item.id === 'trend');

  assert.equal(module.visualization.chartRuntime, 'echarts');
  assert.equal(module.visualization.chartOptions.series[0].type, 'bar');
  assert.deepEqual(module.visualization.chartOptions.series[0].data, [12, 18]);
  assert.equal(next.dataSnapshot.moduleBindings.find((item) => item.moduleId === 'trend').chartRuntime, 'echarts');
});

test('module edit can update chart data rows and feed snapshot and image payload', () => {
  const draft = buildInitialStaticPageDraft();
  const next = applyStaticPageOperation(draft, {
    type: 'update_module',
    targetModuleId: 'trend',
    patch: {
      visualization: {
        type: 'line-chart',
        data: [
          { label: '一月', value: 1200 },
          { label: '二月', value: 1380 },
        ],
      },
    },
  });
  const module = next.modules.find((item) => item.id === 'trend');
  const binding = next.dataSnapshot.moduleBindings.find((item) => item.moduleId === 'trend');
  const imagePayload = buildStaticPageImagePayload(next);

  assert.deepEqual(module.visualization.data, [
    { label: '一月', value: 1200 },
    { label: '二月', value: 1380 },
  ]);
  assert.equal(binding.dataQuality, 'module_data');
  assert.equal(binding.bindingQualityStatus, 'confirmed');
  assert.equal(binding.chartDataFit, 'ready');
  assert.equal(binding.sampleData[0].value, 1200);
  assert.equal(imagePayload.modules.find((item) => item.id === 'trend').sampleData[1].label, '二月');
});

test('change visualization operation can switch chart runtime without losing safe options', () => {
  const draft = buildInitialStaticPageDraft();
  const next = applyStaticPageOperation(draft, {
    type: 'change_visualization',
    targetModuleId: 'trend',
    visualizationType: 'bar-chart',
    chartRuntime: 'echarts',
    chartOptions: {
      xAxis: { type: 'category', data: ['一月'] },
      yAxis: { type: 'value' },
      series: [{ type: 'bar', data: [1200] }],
    },
  });
  const module = next.modules.find((item) => item.id === 'trend');

  assert.equal(module.visualization.type, 'bar-chart');
  assert.equal(module.visualization.chartRuntime, 'echarts');
  assert.equal(module.visualization.chartOptions.series[0].type, 'bar');
});

test('unknown chart runtime falls back to deterministic runtime', () => {
  const draft = buildInitialStaticPageDraft();
  const next = applyStaticPageOperation(draft, {
    type: 'update_module',
    targetModuleId: 'trend',
    patch: {
      visualization: {
        type: 'line-chart',
        chartRuntime: 'plotly',
        chartOptions: {
          series: [{ type: 'line', data: [1, 2] }],
          dataKey: 'orders.amount',
        },
      },
    },
  });
  const module = next.modules.find((item) => item.id === 'trend');

  assert.equal(module.visualization.chartRuntime, 'deterministic');
  assert.equal(module.visualization.chartOptions.dataKey, 'orders.amount');
  assert.equal(module.visualization.chartOptions.series, undefined);
});

test('chart runtime helpers normalize module rows and synthesize ECharts preview options', () => {
  const rows = normalizeStaticPageChartRows([
    { 月份: '一月', 订单金额: '1,200' },
    { label: '二月', value: 1380 },
    { label: '无效', value: 'not-a-number' },
  ]);
  const module = {
    visualization: {
      type: 'line-chart',
      chartRuntime: 'echarts',
      data: rows,
    },
  };
  const option = buildStaticPageEchartsPreviewOption(module);

  assert.deepEqual(rows, [
    { label: '一月', value: 1200 },
    { label: '二月', value: 1380 },
  ]);
  assert.deepEqual(staticPageChartRowsFromModule(module), rows);
  assert.equal(option.series[0].type, 'line');
  assert.deepEqual(option.xAxis.data, ['一月', '二月']);
  assert.equal(hasRenderableEchartsData(option), true);
});

test('unsafe chart options are stripped before reaching module contracts', () => {
  const draft = buildInitialStaticPageDraft();
  const next = applyStaticPageOperation(draft, {
    type: 'update_module',
    targetModuleId: 'trend',
    patch: {
      visualization: {
        type: 'bar-chart',
        chartRuntime: 'echarts',
        chartOptions: {
          tooltip: { formatter: 'javascript:alert(1)' },
          title: { text: '<img src=x onerror=alert(1)>' },
          link: 'https://example.com/evil.js',
          onClick: 'steal()',
          constructor: { prototype: 'pollute' },
          series: [
            {
              type: 'bar',
              data: [1, 2, 3],
              label: { formatter: () => 'unsafe' },
              itemStyle: { color: 'javascript:alert(1)' },
            },
            {
              type: 'custom',
              data: [4],
              renderItem: 'alert(1)',
            },
          ],
        },
      },
    },
  });
  const options = next.modules.find((item) => item.id === 'trend').visualization.chartOptions;
  const serialized = JSON.stringify(options);

  assert.equal(options.series.length, 1);
  assert.equal(options.series[0].type, 'bar');
  assert.equal(options.series[0].label.formatter, undefined);
  assert.equal(options.series[0].itemStyle.color, undefined);
  assert.equal(options.tooltip.formatter, undefined);
  assert.equal(options.title.text, undefined);
  assert.equal(options.link, undefined);
  assert.equal(options.onClick, undefined);
  assert.equal(serialized.includes('javascript:'), false);
  assert.equal(serialized.includes('https://'), false);
  assert.equal(serialized.includes('<img'), false);
});

test('module update operation helper emits full editable binding and chart contract', () => {
  const draft = buildInitialStaticPageDraft({
    datasetId: 'dataset-1',
    sessionId: 'session-1',
    evidenceIds: ['ev-1'],
  });
  const trend = draft.modules.find((module) => module.id === 'trend');
  const operation = buildStaticPageModuleUpdateOperation(trend, {
    title: '订单趋势',
    content: '展示订单金额按月变化。',
    dataBinding: {
      type: 'selected_scope',
      sourceId: 'selected_scope',
      label: '订单金额',
      fieldPath: 'orders.amount',
    },
    visualization: {
      type: 'line-chart',
    },
  });
  const next = applyStaticPageOperation(draft, operation);
  const module = next.modules.find((item) => item.id === 'trend');

  assert.equal(operation.type, 'update_module');
  assert.equal(operation.patch.dataBinding.sourceId, 'selected_scope');
  assert.equal(operation.patch.dataBinding.fieldPath, 'orders.amount');
  assert.equal(operation.patch.visualization.chartOptions.dataKey, 'orders.amount');
  assert.equal(module.dataBinding.sourceId, 'selected_scope');
  assert.equal(module.visualization.chartOptions.showAxis, true);
});

test('data source candidates expose selected datasets evidence and session without forcing forms', () => {
  const draft = buildInitialStaticPageDraft({
    datasetId: 'dataset-1',
    sessionId: 'session-1',
    evidenceIds: ['ev-1'],
  });
  const candidates = buildStaticPageDataSourceCandidates(draft);

  assert.equal(candidates.find((item) => item.sourceId === 'dataset').available, true);
  assert.equal(candidates.find((item) => item.sourceId === 'selected_scope').datasetId, 'dataset-1');
  assert.equal(candidates.find((item) => item.sourceId === 'evidence').evidenceIds[0], 'ev-1');
  assert.equal(candidates.find((item) => item.sourceId === 'session').sessionId, 'session-1');
});

test('field candidates preserve backend suggestions and add evidence fallbacks', () => {
  const draft = buildInitialStaticPageDraft({
    datasetId: 'dataset-1',
    evidenceIds: ['ev-1'],
    fieldCandidates: [
      {
        sourceId: 'evidence',
        field_path: 'orders.amount',
        label: '订单金额',
        kind: 'metric',
        recommended_aggregation: 'sum',
      },
      {
        sourceId: 'evidence',
        fieldPath: 'media.transcript_windows',
        label: '媒体转写时间窗',
        kind: 'media',
        mediaKind: 'audio',
        timestamped: true,
        evidenceIds: ['ev-media'],
        evidenceRef: {
          sourceLocator: 'documents/call.mp3#chunk=0',
        },
      },
    ],
  });
  const candidates = buildStaticPageFieldCandidates(draft);

  assert.equal(candidates.find((item) => item.fieldPath === 'orders.amount').recommendedAggregation, 'sum');
  assert.equal(candidates.find((item) => item.fieldPath === 'media.transcript_windows').timestamped, true);
  assert.equal(candidates.find((item) => item.fieldPath === 'media.transcript_windows').mediaKind, 'audio');
  assert.equal(candidates.find((item) => item.fieldPath === 'media.transcript_windows').evidenceRef.sourceLocator, 'documents/call.mp3#chunk=0');
  assert.ok(candidates.some((item) => item.fieldPath === 'dataset.metrics_summary'));
  assert.ok(candidates.some((item) => item.fieldPath === 'retrieval.content_excerpt'));
});

test('static page visual and render spec builders expose renderer-safe constraints', () => {
  const visualSpec = buildStaticPageVisualSpec('data-command');
  const renderSpec = buildStaticPageRenderSpec();

  assert.equal(visualSpec.styleDirection, 'data-command');
  assert.equal(renderSpec.layoutEngine, 'css-grid-12');
  assert.equal(renderSpec.chartRuntime, 'deterministic-with-echarts-advanced');
  assert.ok(renderSpec.generationGuardrails.some((rule) => rule.includes('DOM')));
});
