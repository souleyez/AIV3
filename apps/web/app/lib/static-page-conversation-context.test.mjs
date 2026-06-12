import assert from 'node:assert/strict';
import test from 'node:test';
import {
  buildAssistantRunSelectedScope,
  buildStaticPageContextFieldCandidates,
  buildStaticPageConversationSummary,
  buildStaticPageDraftSelectedScope,
} from './static-page-conversation-context.js';

test('buildStaticPageConversationSummary composes selected datasets, documents, session, and recent messages', () => {
  const summary = buildStaticPageConversationSummary('生成经营月报', {
    datasets: [
      {
        id: 'dataset-1',
        title: '新世界经营数据',
        document_count: 3,
        parse_status_summary: 'indexed',
      },
      {
        id: 'dataset-2',
        key: 'contract-data',
        documentCount: 2,
      },
    ],
    session: { title: '店总经营分析' },
    documents: [
      { id: 'doc-1', dataset_id: 'dataset-1', title: '取高预警明细' },
      { id: 'doc-2', datasetIds: ['dataset-2'], filename: '合同面积.xlsx' },
      { id: 'doc-out', dataset_id: 'dataset-out', title: '不相关文档' },
    ],
    messages: [
      { role: 'user', content: '忽略的早期消息' },
      { role: 'assistant', content: '这是助手回复' },
      { role: 'user', content: '客户  希望  看  风险门店' },
    ],
  });

  assert.equal(summary, [
    '# 静态页规划输入',
    '用户要求：生成经营月报',
    '数据集：新世界经营数据（3文档）/indexed、contract-data（2文档）',
    '会话：店总经营分析',
    '文档线索：取高预警明细、合同面积.xlsx',
    '上下文：用户: 忽略的早期消息 / 助手: 这是助手回复 / 用户: 客户 希望 看 风险门店',
  ].join('\n'));
  assert.equal(summary.includes('不相关文档'), false);
});

test('buildStaticPageConversationSummary falls back for ordinary chat without datasets', () => {
  const summary = buildStaticPageConversationSummary('', {
    messages: [
      { role: 'user', content: '' },
      { role: 'assistant', content: '   ' },
    ],
  });

  assert.equal(summary, [
    '# 静态页规划输入',
    '数据集：未选数据集，按普通对话意图规划。',
  ].join('\n'));
});

test('buildStaticPageContextFieldCandidates returns de-duplicated dataset and document hints', () => {
  const candidates = buildStaticPageContextFieldCandidates({
    datasets: [
      { id: 'dataset-1', title: '新世界经营数据' },
      { id: 'dataset-2', key: '合同数据' },
    ],
    documents: [
      { dataset_id: 'dataset-1', title: '新世界经营数据' },
      { dataset_id: 'dataset-1', name: '取高预警明细' },
      { datasetIds: ['dataset-2'], filename: '合同面积.xlsx' },
      { dataset_id: 'dataset-out', title: '不相关文档' },
    ],
  });

  assert.equal(candidates.length, 1);
  assert.deepEqual(candidates[0], {
    sourceId: 'evidence',
    fieldPath: 'retrieval.section_title_hints',
    label: '对话和文档标题线索',
    kind: 'section_titles',
    confidence: 0.52,
    sectionTitleHints: ['新世界经营数据', '合同数据', '取高预警明细', '合同面积.xlsx'],
  });
});

test('buildStaticPageContextFieldCandidates returns empty without hints', () => {
  assert.deepEqual(buildStaticPageContextFieldCandidates({ documents: [{ title: '孤立文档' }] }), []);
});

test('static page selected scope helpers preserve existing payload shape', () => {
  assert.deepEqual(buildStaticPageDraftSelectedScope({ datasetId: 'dataset-1' }), {
    mode: 'selected_datasets',
    selected: [{ type: 'dataset', id: 'dataset-1' }],
    datasets: [{ type: 'dataset', id: 'dataset-1' }],
  });
  assert.deepEqual(buildStaticPageDraftSelectedScope({}), {
    mode: 'ordinary_chat',
    selected: [],
  });

  assert.deepEqual(
    buildAssistantRunSelectedScope(['dataset-1', '', 'dataset-1'], {
      intent: 'static_page_report',
      candidates: [{ type: 'conversation_memory' }],
    }),
    {
      mode: 'user_selected',
      datasets: ['dataset-1'],
      selected: [{ type: 'dataset', id: 'dataset-1' }],
      conversation_memory: ['local-thread'],
      intent: 'static_page_report',
      supply_policy: {
        intent: 'static_page_report',
        historyPolicy: 'intent_gated_selected',
        retrievalPolicy: 'standard',
        preferDetail: false,
        noFakeData: true,
      },
    },
  );
  assert.deepEqual(buildAssistantRunSelectedScope([], { supplyStrategy: { intent: 'ordinary_chat', retrievalPolicy: 'not_requested' } }), {
    mode: 'ordinary_chat',
    datasets: [],
    selected: [],
    conversation_memory: [],
    intent: 'ordinary_chat',
    supply_policy: { intent: 'ordinary_chat', retrievalPolicy: 'not_requested' },
  });
});
