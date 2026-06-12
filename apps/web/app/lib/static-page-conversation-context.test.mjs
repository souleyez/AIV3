import assert from 'node:assert/strict';
import test from 'node:test';
import {
  buildAssistantRunSelectedScope,
  buildStaticPageContextFieldCandidates,
  buildStaticPageConversationSummary,
  buildStaticPageDraftSourceRefs,
  buildStaticPageDraftSelectedScope,
  buildStaticPageProgressDescriptor,
  buildStaticPageProgressLocalMessage,
  staticPagePreviewProgressContent,
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

test('buildStaticPageDraftSourceRefs preserves backend source-ref payload shape', () => {
  const refs = buildStaticPageDraftSourceRefs({
    id: 'draft-1',
    localDraftId: 'local-draft-1',
    datasetId: 'dataset-1',
    sessionId: 'session-1',
    source: {
      oneClick: true,
      templateReferences: [{ templateId: 'data-report' }],
    },
  }, {
    localThreadId: 'local-thread-1',
  });

  assert.deepEqual(refs, {
    local_thread_id: 'local-thread-1',
    local_draft_id: 'local-draft-1',
    dataset_id: 'dataset-1',
    chat_session_id: 'session-1',
    source: 'local_chat_static_page_image2_pipeline',
    client_source: {
      oneClick: true,
      templateReferences: [{ templateId: 'data-report' }],
    },
    auto_publish_generated_artifact: true,
    effect_image_confirmation_required: false,
    continue_to_publish_after_effect_image: true,
    fixed_task_template_id: 'static_page_image2_data_publish',
    customer_preview_delivery: 'stream_event_or_status_card',
  });

  assert.deepEqual(buildStaticPageDraftSourceRefs({ id: 'draft-2' }), {
    local_thread_id: '',
    local_draft_id: 'draft-2',
    dataset_id: null,
    chat_session_id: null,
    source: 'local_chat_static_page_image2_pipeline',
    client_source: {},
    auto_publish_generated_artifact: true,
    effect_image_confirmation_required: false,
    continue_to_publish_after_effect_image: true,
    fixed_task_template_id: 'static_page_image2_data_publish',
    customer_preview_delivery: 'stream_event_or_status_card',
  });
});

test('staticPagePreviewProgressContent links only safe preview URLs', () => {
  assert.equal(
    staticPagePreviewProgressContent({
      previewImage: { assetKey: '/generated-artifacts/previews/effect.json' },
    }),
    '设计图已生成：[打开设计图](/generated-artifacts/previews/effect.json)。DataMax 正在继续读取视觉稿并制作最终页面。',
  );
  assert.equal(
    staticPagePreviewProgressContent(
      { previewImage: { assetKey: '/generated-artifacts/previews/older.json' } },
      { previewAssetKey: 'https://v3.elepcloud.com/generated-artifacts/previews/newer.json' },
    ),
    '设计图已生成：[打开设计图](https://v3.elepcloud.com/generated-artifacts/previews/newer.json)。DataMax 正在继续读取视觉稿并制作最终页面。',
  );
  assert.equal(
    staticPagePreviewProgressContent({ previewContract: { assetKey: 'static-page-previews/local.json' } }),
    '设计图已生成。DataMax 正在继续读取视觉稿并制作最终页面。',
  );
});

test('static page progress helpers preserve message key, content, and metadata shape', () => {
  const descriptor = buildStaticPageProgressDescriptor('draft-1:rendered', '  页面已生成  ', { final: true });
  assert.deepEqual(descriptor, {
    stableKey: 'static-page:draft-1:rendered',
    content: '  页面已生成  ',
    final: true,
  });

  const message = buildStaticPageProgressLocalMessage(descriptor, {
    messageFactory: (role, content) => ({
      id: 'message-1',
      role,
      content,
      created_at: '2026-06-12T00:00:00.000Z',
    }),
  });

  assert.deepEqual(message, {
    id: 'message-1',
    role: 'assistant',
    content: '  页面已生成  ',
    created_at: '2026-06-12T00:00:00.000Z',
    metadata: {
      source: 'static_page_progress',
      key: 'static-page:draft-1:rendered',
      final: true,
    },
  });
});

test('static page progress local message tolerates invalid descriptor like the previous caller path', () => {
  assert.equal(buildStaticPageProgressLocalMessage(null), null);
  assert.deepEqual(buildStaticPageProgressDescriptor(null, '', {}), {
    stableKey: 'static-page:null',
    content: '',
    final: false,
  });
});
