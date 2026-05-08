import test from 'node:test';
import assert from 'node:assert/strict';
import { planAssistantScope, selectPlannerDatasetId } from './scope-planner.js';

const datasets = [
  {
    id: 'dataset-orders',
    key: 'orders',
    title: '订单',
    description: '销售、营收、发货和复购数据',
  },
  {
    id: 'dataset-support',
    key: 'support',
    title: '客服',
    description: '客服、工单、投诉和满意度记录',
  },
];

test('scope planner keeps user selected dataset as highest priority', () => {
  const plan = planAssistantScope({
    prompt: '分析一下客服投诉趋势',
    datasets,
    selectedDatasetId: 'dataset-orders',
  });

  assert.equal(plan.candidates[0].id, 'dataset-orders');
  assert.equal(plan.candidates[0].source, 'user_selected');
  assert.equal(selectPlannerDatasetId(plan), 'dataset-orders');
});

test('scope planner preselects matching visible dataset when none is selected', () => {
  const plan = planAssistantScope({
    prompt: '最近订单营收和复购怎么样',
    datasets,
  });

  assert.equal(selectPlannerDatasetId(plan), 'dataset-orders');
  assert.match(plan.hint, /订单/);
  assert.equal(plan.intent, 'data_question');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'standard');
});

test('scope planner can include conversation memory without forcing dataset retrieval', () => {
  const plan = planAssistantScope({
    prompt: '继续按刚才那版改一下',
    datasets,
    conversationMemory: [{ role: 'user', content: '做一个静态页草稿' }],
  });

  assert.equal(plan.candidates.some((candidate) => candidate.type === 'conversation_memory'), true);
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.supplyStrategy.historyPolicy, 'intent_gated_selected');
});

test('scope planner marks static page intent as detail-first when data is available', () => {
  const plan = planAssistantScope({
    prompt: '基于订单做一页静态页经营分析',
    datasets,
  });

  assert.equal(plan.intent, 'static_page');
  assert.equal(plan.intentLabel, '静态页规划');
  assert.equal(selectPlannerDatasetId(plan), 'dataset-orders');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'detail_first');
  assert.equal(plan.supplyStrategy.preferDetail, true);
  assert.match(plan.hint, /意图：静态页规划/);
});

test('scope planner keeps no-dataset static page request as ordinary supply', () => {
  const plan = planAssistantScope({
    prompt: '帮我先规划一页静态页',
    datasets: [],
  });

  assert.equal(plan.intent, 'static_page');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.equal(plan.supplyStrategy.noFakeData, true);
});

test('scope planner treats active static page edits as artifact context', () => {
  const plan = planAssistantScope({
    prompt: '把标题改短，图表换成柱状图',
    datasets,
    activeStaticPageDraft: {
      backendDraftId: 'static-draft-1',
      id: 'local-static-draft-1',
      objective: '订单经营分析页',
    },
  });

  assert.equal(plan.intent, 'static_page');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.candidates.some((candidate) => candidate.type === 'static_page_draft'), true);
  assert.equal(plan.candidates.find((candidate) => candidate.type === 'static_page_draft')?.id, 'static-draft-1');
  assert.equal(plan.supplyStrategy.currentArtifactPolicy, 'active_static_page_draft');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.match(plan.hint, /当前静态页：订单经营分析页/);
});
