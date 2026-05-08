export function buildAssistantStartupBriefing({
  datasets = [],
  reportPlans = [],
  publishedReports = [],
  latestMessages = [],
  activityEvents = [],
  selectedDataset = null,
  activeStaticPageDraft = null,
  staticPageDrafts = [],
} = {}) {
  const visibleDatasets = Array.isArray(datasets) ? datasets : [];
  const reports = Array.isArray(reportPlans) ? reportPlans : [];
  const published = Array.isArray(publishedReports) ? publishedReports : [];
  const messages = Array.isArray(latestMessages) ? latestMessages : [];
  const events = Array.isArray(activityEvents) ? activityEvents : [];
  const staticDrafts = Array.isArray(staticPageDrafts) ? staticPageDrafts : [];
  const staticPageWorkspace = summarizeStaticPageWorkspace(activeStaticPageDraft, staticDrafts);

  const latestActivity = [
    latestActivityEvent(events),
    latestDatasetActivity(visibleDatasets),
    latestMessageActivity(messages),
  ].filter(Boolean)[0] || '暂无最近上传、采集或对话摘要。';

  return {
    briefingVersion: 2,
    productTruth: '这是一个围绕数据集、文档、采集源、检索供料、报表和静态页输出的智能数据工作台。',
    operatingPrinciple: '系统只负责识别意图、检索供料和执行受控动作；最终正文由模型根据 observation 自行回答，不由宿主拼装。',
    visibleDatasetCount: visibleDatasets.length,
    visibleDocumentCount: sumNumericField(visibleDatasets, ['document_count', 'documentCount', 'documents']),
    estimatedWordCount: sumNumericField(visibleDatasets, ['estimated_word_count', 'estimatedWordCount', 'word_count']),
    reportPlanCount: reports.length,
    publishedReportCount: published.length,
    staticPageDraftCount: staticDrafts.length,
    selectedScopeLabel: selectedDataset?.title || '',
    latestActivity,
    parseStateSummary: summarizeParseState(visibleDatasets),
    datasetBriefs: summarizeDatasets(visibleDatasets),
    staticPageWorkspace,
    defaultPublicCategories: ['订单', '客服', '企业问答', '网页采集', '未分类'],
    capabilities: [
      'ordinary_chat',
      'scope_plan',
      'retrieve',
      'read_detail',
      'media_detail',
      'compare',
      'upload_classify',
      'report_plan',
      'static_page_plan',
      'render',
      'controlled_action',
    ],
    productCapabilities: {
      staticPage: '可以在主对话区发起静态页规划、效果图排队、模块编辑、最终静态页渲染，并导出 index.html 与包含 manifest/data/modules/render-spec/README 的交付包。',
      report: '可以让模型主动发起报表/看板创建，但必须先通过工具列出选项并由宿主执行。',
      retrieval: '选中或预选数据集时，宿主会尽量检索相关证据；静态页/报表意图优先深度供料。',
      media: '音视频上传按后台任务解析；有本地转写、场景或关键帧 OCR 时会以可引用证据供料，缺失时保持 partial 而不编造。',
      memory: '本轮对话历史是隐藏数据集，只有用户语义需要上下文时才进入供料。',
    },
  };
}

export function formatStartupBriefingForModel(briefing) {
  const source = briefing || {};
  const parts = [
    source.productTruth,
    `可见数据集 ${Number(source.visibleDatasetCount || 0)} 个，文档 ${Number(source.visibleDocumentCount || 0)} 份，估算字数 ${Number(source.estimatedWordCount || 0)}。`,
    `报表草稿 ${Number(source.reportPlanCount || 0)} 个，已发布 ${Number(source.publishedReportCount || 0)} 个；静态页草稿/成品 ${Number(source.staticPageDraftCount || 0)} 个。`,
    source.selectedScopeLabel ? `当前供料范围：${source.selectedScopeLabel}` : '当前未选数据集，可按普通模型聊天回答。',
    source.operatingPrinciple,
    '系统能力：可普通聊天、资料检索、读取文档细节、读取音视频转写/场景等媒体细节、创建报表、规划/渲染静态页；缺证据时必须说明缺失，不能编造数据。',
    `最近状态：${source.latestActivity || '暂无。'}`,
    `解析状态：${source.parseStateSummary || '暂无解析状态。'}`,
    formatStaticPageWorkspaceForModel(source.staticPageWorkspace),
    source.datasetBriefs?.length
      ? `可见数据集摘要：${source.datasetBriefs.map(formatDatasetBriefForModel).join('；')}`
      : '',
  ];
  return parts.filter(Boolean).join('\n');
}

function sumNumericField(items, fieldNames) {
  return items.reduce((total, item) => {
    const value = fieldNames
      .map((fieldName) => numericFieldValue(item?.[fieldName]))
      .find((candidate) => Number.isFinite(candidate) && candidate > 0);
    return total + (value || 0);
  }, 0);
}

function numericFieldValue(value) {
  if (Array.isArray(value)) return value.length;
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function latestDatasetActivity(datasets) {
  const latest = [...datasets]
    .filter((dataset) => dataset?.updated_at || dataset?.created_at)
    .sort((left, right) => {
      const leftTime = new Date(left.updated_at || left.created_at || 0).getTime();
      const rightTime = new Date(right.updated_at || right.created_at || 0).getTime();
      return rightTime - leftTime;
    })[0];
  return latest ? `最近更新数据集：${latest.title || latest.key || '未命名数据集'}` : '';
}

function latestMessageActivity(messages) {
  const latest = [...messages]
    .filter((message) => message?.content)
    .reverse()[0];
  if (!latest) return '';
  return `最近对话：${String(latest.content).replace(/\s+/g, ' ').slice(0, 80)}`;
}

function latestActivityEvent(events) {
  const latest = [...events]
    .filter((event) => event?.summary)
    .sort((left, right) => {
      const leftTime = new Date(left.created_at || 0).getTime();
      const rightTime = new Date(right.created_at || 0).getTime();
      return rightTime - leftTime;
    })[0];
  return latest ? latest.summary : '';
}

function summarizeParseState(datasets) {
  if (!datasets.length) {
    return '当前可见库为空，等待上传或采集。';
  }
  const lifecycleCounts = datasets.reduce((counts, dataset) => {
    const key = String(dataset?.lifecycle || dataset?.status || 'unknown');
    counts[key] = (counts[key] || 0) + 1;
    return counts;
  }, {});
  return Object.entries(lifecycleCounts)
    .map(([key, count]) => `${key}:${count}`)
    .join('，');
}

function summarizeDatasets(datasets) {
  return datasets.slice(0, 8).map((dataset) => ({
    id: dataset?.id || '',
    key: dataset?.key || '',
    title: dataset?.title || dataset?.key || '未命名数据集',
    description: String(dataset?.description || '').slice(0, 80),
    visibility: dataset?.visibility || dataset?.access || '',
    category: dataset?.category || dataset?.default_category || '',
    lifecycle: dataset?.lifecycle || dataset?.status || 'unknown',
    parseStatusSummary: datasetParseStatusSummary(dataset),
    materialHints: datasetMaterialHints(dataset),
    documentCount: datasetDocumentCount(dataset),
    estimatedWordCount: datasetEstimatedWordCount(dataset),
    updatedAt: dataset?.updated_at || dataset?.updatedAt || '',
    latestActivity: dataset?.latestUpload || dataset?.latest_upload || dataset?.updated_at || dataset?.updatedAt || '',
  }));
}

function datasetDocumentCount(dataset = {}) {
  return numericFieldValue(dataset.document_count)
    || numericFieldValue(dataset.documentCount)
    || numericFieldValue(dataset.documents_count)
    || numericFieldValue(dataset.documentsCount)
    || numericFieldValue(dataset.documents);
}

function datasetEstimatedWordCount(dataset = {}) {
  return numericFieldValue(dataset.estimated_word_count)
    || numericFieldValue(dataset.estimatedWordCount)
    || numericFieldValue(dataset.word_count)
    || numericFieldValue(dataset.wordCount);
}

function datasetMaterialHints(dataset = {}) {
  const haystack = `${dataset.title || ''} ${dataset.key || ''} ${dataset.description || ''} ${dataset.category || ''} ${dataset.default_category || ''}`;
  const hints = new Set(
    Array.isArray(dataset.materialHints)
      ? dataset.materialHints
      : Array.isArray(dataset.material_hints)
        ? dataset.material_hints
        : [],
  );
  if (/音视频|音频|视频|录音|转写|字幕|会议|访谈|关键帧|ocr/i.test(haystack)) {
    hints.add('audio_video');
    hints.add('transcript_possible');
    hints.add('keyframe_ocr_possible');
  }
  return [...hints].filter((hint) => typeof hint === 'string' && hint.trim()).slice(0, 6);
}

function datasetParseStatusSummary(dataset = {}) {
  const explicit = dataset.parse_status_summary || dataset.parseStatusSummary || dataset.parse_status || dataset.parseStatus;
  if (explicit) return String(explicit).slice(0, 80);
  const documents = Array.isArray(dataset.documents) ? dataset.documents : [];
  if (!documents.length) return dataset.lifecycle || dataset.status || 'unknown';
  const counts = documents.reduce((acc, document) => {
    const key = document?.parseStatus || document?.parse_status || document?.status || 'unknown';
    acc[key] = (acc[key] || 0) + 1;
    return acc;
  }, {});
  return Object.entries(counts)
    .map(([key, count]) => `${key}:${count}`)
    .join('，')
    .slice(0, 80);
}

function formatDatasetBriefForModel(item) {
  const hints = Array.isArray(item.materialHints) && item.materialHints.length
    ? `/${item.materialHints.join('+')}`
    : '';
  const parse = item.parseStatusSummary && item.parseStatusSummary !== item.lifecycle
    ? `/解析:${item.parseStatusSummary}`
    : '';
  return `${item.title}(${item.documentCount}文档/${item.lifecycle}${parse}${hints})`;
}

function summarizeStaticPageWorkspace(activeDraft, drafts) {
  const draftList = Array.isArray(drafts) ? drafts : [];
  const active = activeDraft && typeof activeDraft === 'object' ? activeDraft : null;
  const activeModules = Array.isArray(active?.modules) ? active.modules : [];
  const echartsModules = activeModules
    .filter((module) => module?.visualization?.chartRuntime === 'echarts')
    .length;
  const finalStatus = active?.finalPage?.status || '';
  const previewStatus = active?.previewContract?.status || active?.imageJob?.status || '';
  return {
    activeDraftId: active?.backendDraftId || active?.id || '',
    activeDraftStatus: active?.status || '',
    activeDraftObjective: String(active?.objective || active?.title || '').slice(0, 120),
    activeStyleDirection: active?.styleDirection || '',
    activeModuleCount: activeModules.length,
    activeEchartsModuleCount: echartsModules,
    activeDeterministicModuleCount: Math.max(0, activeModules.length - echartsModules),
    previewStatus,
    finalRenderStatus: finalStatus,
    canEditModules: Boolean(active && activeModules.length),
    canExportFinal: finalStatus === 'rendered',
    latestDrafts: draftList.slice(0, 5).map((draft) => ({
      id: draft?.backendDraftId || draft?.id || '',
      title: String(draft?.objective || draft?.title || '静态页草稿').slice(0, 80),
      status: draft?.finalPage?.status || draft?.status || draft?.backendStatus || 'draft',
      moduleCount: Array.isArray(draft?.modules) ? draft.modules.length : 0,
    })),
  };
}

function formatStaticPageWorkspaceForModel(workspace) {
  if (!workspace || typeof workspace !== 'object') {
    return '';
  }
  if (!workspace.activeDraftId && !workspace.latestDrafts?.length) {
    return '静态页工作区：当前没有打开草稿，也没有可见静态页成品。';
  }
  const parts = [];
  if (workspace.activeDraftId) {
    parts.push(
      `当前静态页：${workspace.activeDraftObjective || workspace.activeDraftId}，状态 ${workspace.activeDraftStatus || 'unknown'}，风格 ${workspace.activeStyleDirection || '未定'}，模块 ${workspace.activeModuleCount || 0} 个，ECharts ${workspace.activeEchartsModuleCount || 0} 个，效果图 ${workspace.previewStatus || 'none'}，最终渲染 ${workspace.finalRenderStatus || 'none'}。`,
    );
    if (workspace.canEditModules) {
      parts.push('用户要求调整静态页时，应优先围绕当前打开草稿做模块级标题、内容、数据、图表、布局或风格变更。');
    }
    if (workspace.canExportFinal) {
      parts.push('当前静态页已可导出 index.html 和交付包。');
    }
  }
  if (workspace.latestDrafts?.length) {
    parts.push(`最近静态页：${workspace.latestDrafts.map((draft) => `${draft.title}(${draft.status})`).join('；')}`);
  }
  return `静态页工作区：${parts.join(' ')}`;
}
