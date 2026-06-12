import {
  documentDatasetIds,
  normalizeDatasetIds,
} from './dataset-record-scope.js';

export function buildStaticPageConversationSummary(prompt = '', options = {}) {
  const draftDataset = options.dataset || null;
  const draftDatasets = Array.isArray(options.datasets) && options.datasets.length
    ? options.datasets
    : [];
  const draftSession = options.session || null;
  const sourceMessages = Array.isArray(options.messages) ? options.messages : [];
  const documents = Array.isArray(options.documents) ? options.documents : [];
  const draftDatasetIds = new Set([
    ...draftDatasets.map((dataset) => dataset.id).filter(Boolean),
    draftDataset?.id,
  ].filter(Boolean));
  const relatedDocuments = documents
    .filter((document) => documentDatasetIds(document).some((datasetId) => draftDatasetIds.has(datasetId)))
    .slice(0, 8);
  const recentMessages = sourceMessages
    .slice(-6)
    .map((message) => {
      const role = message.role === 'assistant' ? '助手' : '用户';
      return `${role}: ${String(message.content || '').replace(/\s+/g, ' ').trim().slice(0, 180)}`;
    })
    .filter((line) => !/^(助手|用户):\s*$/.test(line));
  const datasetLine = draftDatasets.length
    ? draftDatasets.map((dataset) => {
        const count = dataset.document_count ?? dataset.documentCount ?? dataset.documents_count ?? dataset.documentsCount;
        const status = dataset.parse_status_summary || dataset.parseStatusSummary || dataset.content_type_summary || dataset.contentTypeSummary || '';
        return `${dataset.title || dataset.key}${count ? `（${count}文档）` : ''}${status ? `/${status}` : ''}`;
      }).join('、')
    : draftDataset ? `${draftDataset.title || draftDataset.key}` : '未选数据集，按普通对话意图规划。';
  const documentLine = relatedDocuments
    .map((document) => document.title || document.name || document.filename || document.id)
    .filter(Boolean)
    .slice(0, 8)
    .join('、');
  const summaryParts = [
    '# 静态页规划输入',
    prompt ? `用户要求：${prompt}` : '',
    `数据集：${datasetLine}`,
    draftSession ? `会话：${draftSession.title}` : '',
    documentLine ? `文档线索：${documentLine}` : '',
    recentMessages.length ? `上下文：${recentMessages.join(' / ')}` : '',
  ].filter(Boolean);
  return summaryParts.join('\n');
}

export function buildStaticPageContextFieldCandidates(options = {}) {
  const draftDataset = options.dataset || null;
  const draftDatasets = Array.isArray(options.datasets) && options.datasets.length
    ? options.datasets
    : [];
  const documents = Array.isArray(options.documents) ? options.documents : [];
  const datasetIds = new Set([
    ...draftDatasets.map((dataset) => dataset.id).filter(Boolean),
    draftDataset?.id,
  ].filter(Boolean));
  const hints = [];
  draftDatasets.forEach((dataset) => {
    if (dataset?.title || dataset?.key) hints.push(dataset.title || dataset.key);
  });
  if (!draftDatasets.length && (draftDataset?.title || draftDataset?.key)) {
    hints.push(draftDataset.title || draftDataset.key);
  }
  documents
    .filter((document) => documentDatasetIds(document).some((datasetId) => datasetIds.has(datasetId)))
    .slice(0, 10)
    .forEach((document) => {
      const title = document.title || document.name || document.filename;
      if (title && !hints.includes(title)) hints.push(title);
    });
  if (!hints.length) return [];
  return [{
    sourceId: 'evidence',
    fieldPath: 'retrieval.section_title_hints',
    label: '对话和文档标题线索',
    kind: 'section_titles',
    confidence: 0.52,
    sectionTitleHints: hints.slice(0, 12),
  }];
}

export function buildStaticPageDraftSelectedScope(draft) {
  if (draft?.datasetId) {
    return {
      mode: 'selected_datasets',
      selected: [{ type: 'dataset', id: draft.datasetId }],
      datasets: [{ type: 'dataset', id: draft.datasetId }],
    };
  }
  return {
    mode: 'ordinary_chat',
    selected: [],
  };
}

export function buildAssistantRunSelectedScope(datasetIds, scopePlan) {
  const scopeDatasetIds = normalizeDatasetIds(datasetIds);
  const conversationMemory = (scopePlan?.candidates || []).some((candidate) => candidate.type === 'conversation_memory')
    ? ['local-thread']
    : [];
  const intent = scopePlan?.intent || scopePlan?.supplyStrategy?.intent || 'ordinary_chat';
  const supplyPolicy = scopePlan?.supplyStrategy || {
    intent,
    historyPolicy: conversationMemory.length ? 'intent_gated_selected' : 'intent_gated',
    retrievalPolicy: scopeDatasetIds.length ? 'standard' : 'not_requested',
    preferDetail: false,
    noFakeData: true,
  };
  if (scopeDatasetIds.length) {
    return {
      mode: 'user_selected',
      datasets: scopeDatasetIds,
      selected: scopeDatasetIds.map((datasetId) => ({ type: 'dataset', id: datasetId })),
      conversation_memory: conversationMemory,
      intent,
      supply_policy: supplyPolicy,
    };
  }
  return {
    mode: 'ordinary_chat',
    datasets: [],
    selected: [],
    conversation_memory: conversationMemory,
    intent,
    supply_policy: supplyPolicy,
  };
}

export function buildStaticPageDraftSourceRefs(draft, options = {}) {
  return {
    local_thread_id: options.localThreadId || '',
    local_draft_id: draft?.localDraftId || draft?.id || '',
    dataset_id: draft?.datasetId || null,
    chat_session_id: draft?.sessionId || null,
    source: 'local_chat_static_page_image2_pipeline',
    client_source: draft?.source || {},
    auto_publish_generated_artifact: true,
    effect_image_confirmation_required: false,
    continue_to_publish_after_effect_image: true,
    fixed_task_template_id: 'static_page_image2_data_publish',
    customer_preview_delivery: 'stream_event_or_status_card',
  };
}

export function staticPagePreviewProgressContent(draft, snapshot = {}) {
  const previewUrl = String(
    snapshot.previewAssetKey
      || draft?.previewImage?.assetKey
      || draft?.previewContract?.assetKey
      || '',
  ).trim();
  const link = /^(https?:\/\/|\/)/i.test(previewUrl)
    ? `：[打开设计图](${previewUrl})`
    : '';
  return `设计图已生成${link}。DataMax 正在继续读取视觉稿并制作最终页面。`;
}
