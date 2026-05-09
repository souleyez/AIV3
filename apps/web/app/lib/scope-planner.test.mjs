import test from 'node:test';
import assert from 'node:assert/strict';
import { planAssistantScope, selectPlannerDatasetId } from './scope-planner.js';

const datasets = [
  {
    id: 'dataset-orders',
    key: 'orders',
    title: '订单',
    description: '销售、营收、发货和复购数据',
    lifecycle: 'active',
    document_count: 12,
    estimated_word_count: 3600,
    parse_status_summary: 'completed:12',
  },
  {
    id: 'dataset-support',
    key: 'support',
    title: '客服',
    description: '客服、工单、投诉和满意度记录',
    lifecycle: 'ingesting',
    documentCount: 4,
    wordCount: 900,
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
  assert.match(plan.hint, /12文档/);
  assert.equal(plan.intent, 'data_question');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'standard');
  assert.equal(plan.supplyStrategy.candidatePolicy, 'selected_or_inferred_visible_datasets_only');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['retrieval.search']);
  assert.equal(plan.candidates[0].documentCount, 12);
  assert.equal(plan.candidates[0].estimatedWordCount, 3600);
  assert.equal(plan.candidates[0].parseStatusSummary, 'completed:12');
});

test('scope planner keeps unrelated no-dataset chat as ordinary model chat', () => {
  const plan = planAssistantScope({
    prompt: '帮我写一句开场白',
    datasets,
    conversationMemory: [{ role: 'user', content: '刚才讨论了订单风险' }],
  });

  assert.equal(plan.intent, 'ordinary_chat');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.candidates.length, 0);
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.equal(plan.supplyStrategy.preferDetail, false);
  assert.equal(plan.supplyStrategy.historyPolicy, 'intent_gated');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['ordinary_chat.answer']);
});

test('scope planner preselects media dataset for audio and video prompts', () => {
  const plan = planAssistantScope({
    prompt: '这段录音讲了什么，帮我提炼重点',
    datasets: [
      ...datasets,
      {
        id: 'dataset-media',
        key: 'meeting-media',
        title: '会议录音',
        description: '上传的音频、视频、转写和关键帧 OCR 资料',
      },
    ],
  });

  assert.equal(selectPlannerDatasetId(plan), 'dataset-media');
  assert.equal(plan.intent, 'data_question');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'detail_first');
  assert.equal(plan.supplyStrategy.preferDetail, true);
  assert.equal(plan.supplyStrategy.contextBudgetPolicy, 'quality_first_token_tolerant');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['retrieval.search', 'retrieval.read_detail', 'media.detail']);
  assert.deepEqual(
    plan.candidates.find((candidate) => candidate.id === 'dataset-media')?.materialHints,
    ['audio_video', 'transcript_possible', 'keyframe_ocr_possible'],
  );
  assert.match(plan.hint, /会议录音/);
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
  assert.equal(plan.supplyStrategy.contextBudgetPolicy, 'quality_first_token_tolerant');
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
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['retrieval.search', 'retrieval.read_detail', 'static_page.plan']);
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
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['static_page.plan']);
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
      status: 'planning',
      styleDirection: 'data-command',
      previewContract: { status: 'stale' },
      finalPage: null,
      modules: [{ id: 'hero' }, { id: 'trend' }],
    },
  });

  const draftCandidate = plan.candidates.find((candidate) => candidate.type === 'static_page_draft');
  assert.equal(plan.intent, 'static_page');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.candidates.some((candidate) => candidate.type === 'static_page_draft'), true);
  assert.equal(draftCandidate?.id, 'static-draft-1');
  assert.equal(draftCandidate?.moduleCount, 2);
  assert.equal(draftCandidate?.previewStatus, 'stale');
  assert.equal(draftCandidate?.previewStale, true);
  assert.equal(plan.supplyStrategy.currentArtifactPolicy, 'active_static_page_draft');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['static_page.update_draft']);
  assert.match(plan.hint, /当前静态页：订单经营分析页\(规划已变更\)/);
});
