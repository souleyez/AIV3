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
  assert.equal(briefing.selectedScopeLabel, '订单数据');
  assert.match(briefing.latestActivity, /订单数据/);
  assert.match(briefing.productTruth, /智能数据工作台/);
});

test('formatted briefing tells model when no dataset is selected', () => {
  const briefing = buildAssistantStartupBriefing();
  const formatted = formatStartupBriefingForModel(briefing);

  assert.match(formatted, /当前未选数据集/);
  assert.match(formatted, /普通模型聊天/);
  assert.match(formatted, /当前可见库为空/);
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
