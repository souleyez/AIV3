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
});

test('scope planner can include conversation memory without forcing dataset retrieval', () => {
  const plan = planAssistantScope({
    prompt: '继续按刚才那版改一下',
    datasets,
    conversationMemory: [{ role: 'user', content: '做一个静态页草稿' }],
  });

  assert.equal(plan.candidates.some((candidate) => candidate.type === 'conversation_memory'), true);
  assert.equal(selectPlannerDatasetId(plan), '');
});
