import test from 'node:test';
import assert from 'node:assert/strict';
import { buildAssistantStartupBriefing, formatStartupBriefingForModel } from './assistant-startup-briefing.js';

test('startup briefing summarizes visible datasets and system capability', () => {
  const briefing = buildAssistantStartupBriefing({
    datasets: [
      {
        id: 'dataset-1',
        key: 'orders',
        title: '订单数据',
        lifecycle: 'active',
        document_count: 3,
        estimated_word_count: 1200,
        parse_status_summary: 'completed:3',
        materialHints: ['tabular'],
        updated_at: '2026-04-26T10:00:00.000Z',
      },
      {
        id: 'dataset-2',
        key: 'support',
        title: '客服知识库',
        lifecycle: 'ingesting',
        documentCount: 2,
        word_count: 800,
        updated_at: '2026-04-25T10:00:00.000Z',
      },
    ],
    reportPlans: [{ id: 'plan-1' }],
    publishedReports: [{ id: 'report-1' }],
    selectedDataset: { title: '订单数据' },
  });

  assert.equal(briefing.visibleDatasetCount, 2);
  assert.equal(briefing.visibleDocumentCount, 5);
  assert.equal(briefing.estimatedWordCount, 2000);
  assert.equal(briefing.reportPlanCount, 1);
  assert.equal(briefing.publishedReportCount, 1);
  assert.equal(briefing.staticPageDraftCount, 0);
  assert.equal(briefing.selectedScopeLabel, '订单数据');
  assert.equal(briefing.briefingVersion, 2);
  assert.equal(briefing.datasetBriefs.length, 2);
  assert.equal(briefing.datasetBriefs[0].parseStatusSummary, 'completed:3');
  assert.deepEqual(briefing.datasetBriefs[0].materialHints, ['tabular']);
  assert.ok(briefing.capabilities.includes('media_detail'));
  assert.ok(briefing.capabilities.includes('continuous_execution'));
  assert.match(briefing.productCapabilities.staticPage, /静态页规划/);
  assert.match(briefing.productCapabilities.staticPage, /index\.html/);
  assert.match(briefing.productCapabilities.staticPage, /ZIP 交付包/);
  assert.match(briefing.productCapabilities.media, /partial/);
  assert.match(briefing.productCapabilities.continuousExecution, /受控动作/);
  assert.match(briefing.latestActivity, /订单数据/);
  assert.match(briefing.productTruth, /智能数据工作台/);
});

test('formatted briefing tells model when no dataset is selected', () => {
  const briefing = buildAssistantStartupBriefing();
  const formatted = formatStartupBriefingForModel(briefing);

  assert.match(formatted, /当前未选数据集/);
  assert.match(formatted, /普通模型聊天/);
  assert.match(formatted, /创建报表/);
  assert.match(formatted, /媒体细节/);
  assert.match(formatted, /规划\/渲染\/修改静态页/);
  assert.match(formatted, /导出静态页 ZIP 交付包/);
  assert.match(formatted, /连续提出检索/);
  assert.match(formatted, /静态页草稿\/成品 0 个/);
  assert.match(formatted, /不能编造数据/);
  assert.match(formatted, /当前可见库为空/);
});

test('formatted briefing includes compact dataset material metadata without content', () => {
  const briefing = buildAssistantStartupBriefing({
    datasets: [{
      id: 'dataset-media',
      key: 'media',
      title: '会议录音',
      lifecycle: 'active',
      documents: [
        { id: 'doc-1', parseStatus: 'completed', content: '正文不能进入简报' },
        { id: 'doc-2', parseStatus: 'partial', content: '转写原文也不能进入简报' },
      ],
      description: '音频、视频、转写和关键帧 OCR',
    }],
  });
  const formatted = formatStartupBriefingForModel(briefing);

  assert.equal(briefing.visibleDocumentCount, 2);
  assert.equal(briefing.datasetBriefs[0].documentCount, 2);
  assert.equal(briefing.datasetBriefs[0].parseStatusSummary, 'completed:1，partial:1');
  assert.ok(briefing.datasetBriefs[0].materialHints.includes('audio_video'));
  assert.match(formatted, /会议录音\(2文档\/active\/解析:completed:1，partial:1\/audio_video/);
  assert.doesNotMatch(formatted, /正文不能进入简报/);
  assert.doesNotMatch(formatted, /转写原文/);
});

test('startup briefing prefers recent upload or classification activity', () => {
  const briefing = buildAssistantStartupBriefing({
    datasets: [
      {
        id: 'dataset-1',
        key: 'orders',
        title: '订单数据',
        lifecycle: 'active',
        updated_at: '2026-04-25T10:00:00.000Z',
      },
    ],
    activityEvents: [
      {
        summary: '上传分类：客服投诉明细.xlsx -> 客服',
        created_at: '2026-04-26T10:00:00.000Z',
      },
    ],
  });

  assert.match(briefing.latestActivity, /客服投诉明细/);
});

test('startup briefing includes compact static page workspace context', () => {
  const briefing = buildAssistantStartupBriefing({
    activeStaticPageDraft: {
      id: 'local-draft-1',
      backendDraftId: 'draft-backend-1',
      objective: '客户经营分析静态页',
      status: 'rendered',
      styleDirection: 'data-command',
      previewContract: { status: 'confirmed' },
      finalPage: { status: 'rendered' },
      modules: [
        {
          id: 'trend',
          title: '趋势',
          content: '不应该完整依赖 content 进入工作区摘要',
          visualization: { chartRuntime: 'echarts' },
        },
        {
          id: 'risk',
          title: '风险',
          visualization: { chartRuntime: 'deterministic' },
        },
      ],
    },
    staticPageDrafts: [
      {
        id: 'local-draft-1',
        objective: '客户经营分析静态页',
        status: 'rendered',
        modules: [{ id: 'trend' }],
      },
    ],
  });
  const formatted = formatStartupBriefingForModel(briefing);

  assert.equal(briefing.staticPageDraftCount, 1);
  assert.equal(briefing.staticPageWorkspace.activeDraftId, 'draft-backend-1');
  assert.equal(briefing.staticPageWorkspace.activeModuleCount, 2);
  assert.equal(briefing.staticPageWorkspace.activeEchartsModuleCount, 1);
  assert.equal(briefing.staticPageWorkspace.canExportFinal, true);
  assert.match(formatted, /当前静态页：客户经营分析静态页/);
  assert.match(formatted, /ECharts 1 个/);
  assert.match(formatted, /已可导出 index\.html/);
  assert.doesNotMatch(formatted, /不应该完整依赖 content/);
});
