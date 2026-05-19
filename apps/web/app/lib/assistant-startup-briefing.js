import { attachVisibleDocumentsToDatasets, datasetDocumentTitleHints } from './dataset-document-hints.js';

const MEDIA_EXTRACTION_POLICY = {
  supportedSources: [
    '上传视频文件',
    '直接视频 URL',
    '可公开访问且可解析视频地址的页面',
  ],
  unsupportedSources: [
    '登录态页面',
    '扫码登录',
    'Cookie/Session 复用',
    '录屏绕过',
    '私有或付费内容',
  ],
  modelRequestActions: ['media.resolve_video_url', 'media.extract_ppt_transcript'],
  controlledPipeline: [
    'media.resolve_video_url',
    'media.register_video_asset',
    'media.extract_ppt_transcript',
  ],
  outputArtifacts: [
    'video_source_resolution',
    'transcript_text',
    'ppt_outline',
    'timestamp_map',
    'slide_candidates_manifest',
    'selected_slides_manifest',
    'slide_notes',
    'video_slides_markdown',
    'subtitle_page_map',
    'final_deliverables_manifest',
    'published_deliverable_manifest',
    'published_version_history',
    'extraction_artifacts_manifest',
    'video_slides.md',
    'video_slides_screenshot_based.pptx',
  ],
  accessRule: '未收到 V3 observation 确认视频源解析、转写、抽帧或 PPT 产物前，不要声称已访问视频或看过视频内容。',
  evidenceRule: '转写、OCR、场景切片、时间戳或 subtitle_page_map 缺失时，保持 partial 并说明缺失项。',
};

const MODEL_AWARENESS_POLICY = {
  identity: '你正在服务 AI Data Platform V3。V3 是数据集、第三方知识库、权限、检索供料、受控动作、报表和静态页产物的统一工作台。',
  additiveContextRule: 'V3 上下文是附加能力，不是能力限制。即使当前没有可见数据集或供料，也可以保持通用模型水准回答普通问题。',
  unavailableEvidenceRule: '涉及 V3 数据、文档、权限、工具结果或产物状态时，只有收到 V3 observation/供料才能当作事实。未供料时先说明“当前不可见/未供料”，再区分通用判断。',
  externalSearchPolicy: {
    status: 'planned_v3_controlled_read_only',
    modelRule: '外部/网页搜索是计划中的 V3 受控只读能力；未收到带来源和时间的 V3 search evidence 前，不要声称已联网搜索或引用实时网页结果。',
  },
};

const SUPPLY_EVIDENCE_POLICY = {
  citableEvidenceRule: '只有 V3 supplied_items、observation、retrieval evidence、document detail 或 search evidence 可以作为可引用事实。',
  planningOnlyRule: 'scope candidates、dataset briefs、startup briefing、detail_targets 只用于规划检索或细读，不是引用依据。',
  detailTargetRule: 'detail_targets 代表建议深读目标；未调用 read_document_detail 或收到 observation 前，不要把目标文档内容当作事实。',
};

const STATIC_PAGE_VISUALIZATIONS_REQUIRING_SAMPLE_ROWS = new Set([
  'kpi-cards',
  'bar-chart',
  'line-chart',
  'donut-chart',
  'table',
  'risk-matrix',
]);

const STATIC_PAGE_READY_BINDING_STATUSES = new Set(['confirmed', 'ready', 'non_chart']);
const STATIC_PAGE_READY_CHART_FITS = new Set(['ready', 'not_required', 'non_chart_ready']);

export function buildAssistantStartupBriefing({
  datasets = [],
  documents = [],
  reportPlans = [],
  publishedReports = [],
  latestMessages = [],
  activityEvents = [],
  selectedDataset = null,
  selectedDatasets = [],
  activeStaticPageDraft = null,
  staticPageDrafts = [],
} = {}) {
  const visibleDatasets = attachVisibleDocumentsToDatasets(datasets, documents);
  const reports = Array.isArray(reportPlans) ? reportPlans : [];
  const published = Array.isArray(publishedReports) ? publishedReports : [];
  const messages = Array.isArray(latestMessages) ? latestMessages : [];
  const events = Array.isArray(activityEvents) ? activityEvents : [];
  const staticDrafts = Array.isArray(staticPageDrafts) ? staticPageDrafts : [];
  const selectedScope = Array.isArray(selectedDatasets) && selectedDatasets.length
    ? selectedDatasets
    : selectedDataset
      ? [selectedDataset]
      : [];
  const staticPageWorkspace = summarizeStaticPageWorkspace(activeStaticPageDraft, staticDrafts);

  const latestActivity = [
    latestActivityEvent(events),
    latestDatasetActivity(visibleDatasets),
    latestMessageActivity(messages),
  ].filter(Boolean)[0] || '暂无最近上传、采集或对话摘要。';

  return {
    briefingVersion: 3,
    productTruth: '这是一个围绕数据集、文档、采集源、检索供料、报表和静态页输出的智能数据工作台。',
    operatingPrinciple: '系统只负责识别意图、检索供料和执行受控动作；最终正文由模型根据 observation 自行回答，不由宿主拼装。',
    visibleDatasetCount: visibleDatasets.length,
    visibleDocumentCount: sumNumericField(visibleDatasets, ['document_count', 'documentCount', 'documents']),
    estimatedWordCount: sumNumericField(visibleDatasets, ['estimated_word_count', 'estimatedWordCount', 'word_count']),
    reportPlanCount: reports.length,
    publishedReportCount: published.length,
    staticPageDraftCount: staticDrafts.length,
    selectedScopeLabel: selectedScope.map((dataset) => dataset?.title || dataset?.key).filter(Boolean).join('、'),
    selectedScopeCount: selectedScope.length,
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
      'video_url_resolve',
      'video_ppt_extract',
      'compare',
      'upload_classify',
      'report_plan',
      'static_page_plan',
      'render',
      'controlled_action',
      'continuous_execution',
    ],
    mediaExtractionPolicy: buildMediaExtractionPolicy(),
    productCapabilities: {
      staticPage: '可以在主对话区发起静态页规划、效果图排队、模块编辑、最终静态页渲染，并导出 index.html 与包含 manifest/data/modules/render-spec/runtime/README 的 ZIP 交付包。',
      report: '可以让模型主动发起报表/看板创建，但必须先通过工具列出选项并由宿主执行。',
      retrieval: '选中或预选数据集时，宿主会尽量检索相关证据；静态页/报表意图优先深度供料。',
      media: '音视频上传按后台任务解析；支持上传视频文件、直接视频 URL、公开页面可解析视频地址后的 PPT/原文提取；完整视频/PPT 包应输出截图型 PPTX、video_slides.md、讲稿备注、字幕对页和清单类文件；有本地转写、场景或关键帧 OCR 时会以可引用证据供料，缺失时保持 partial 而不编造；登录态、扫码、Cookie 或录屏绕过不在当前自动能力范围。',
      memory: '本轮对话历史是隐藏数据集，只有用户语义需要上下文时才进入供料。',
      continuousExecution: '模型可以连续提出检索、细读、静态页规划/修改、报表规划、渲染或导出等受控动作；宿主负责校验权限、执行动作并把简要步骤回写到对话。',
    },
    modelAwarenessPolicy: buildModelAwarenessPolicy(),
    supplyEvidencePolicy: buildSupplyEvidencePolicy(),
  };
}

export function formatStartupBriefingForModel(briefing) {
  const source = briefing || {};
  const parts = [
    source.productTruth,
    formatModelAwarenessPolicyForModel(source.modelAwarenessPolicy),
    formatSupplyEvidencePolicyForModel(source.supplyEvidencePolicy),
    `可见数据集 ${Number(source.visibleDatasetCount || 0)} 个，文档 ${Number(source.visibleDocumentCount || 0)} 份，估算字数 ${Number(source.estimatedWordCount || 0)}。`,
    `报表草稿 ${Number(source.reportPlanCount || 0)} 个，已发布 ${Number(source.publishedReportCount || 0)} 个；静态页草稿/成品 ${Number(source.staticPageDraftCount || 0)} 个。`,
    source.selectedScopeLabel ? `当前供料范围：${source.selectedScopeLabel}` : '当前未选数据集，可按普通模型聊天回答。',
    source.operatingPrinciple,
    '系统能力：可普通聊天、资料检索、读取文档细节、读取音视频转写/场景等媒体细节、上传或公开视频 URL 的视频转 PPT/原文提取、创建报表、规划/渲染/修改静态页、导出静态页 ZIP 交付包；缺证据时必须说明缺失，不能编造数据。',
    formatMediaExtractionPolicyForModel(source.mediaExtractionPolicy),
    source.productCapabilities?.continuousExecution || '',
    `最近状态：${source.latestActivity || '暂无。'}`,
    `解析状态：${source.parseStateSummary || '暂无解析状态。'}`,
    formatStaticPageWorkspaceForModel(source.staticPageWorkspace),
    source.datasetBriefs?.length
      ? `可见数据集摘要：${source.datasetBriefs.map(formatDatasetBriefForModel).join('；')}`
      : '',
  ];
  return parts.filter(Boolean).join('\n');
}

function buildModelAwarenessPolicy() {
  return {
    ...MODEL_AWARENESS_POLICY,
    externalSearchPolicy: { ...MODEL_AWARENESS_POLICY.externalSearchPolicy },
  };
}

function buildSupplyEvidencePolicy() {
  return { ...SUPPLY_EVIDENCE_POLICY };
}

function formatSupplyEvidencePolicyForModel(policy) {
  if (!policy || typeof policy !== 'object') {
    return '';
  }
  return [
    policy.citableEvidenceRule || '',
    policy.planningOnlyRule || '',
    policy.detailTargetRule || '',
  ].filter(Boolean).join(' ');
}

function formatModelAwarenessPolicyForModel(policy) {
  if (!policy || typeof policy !== 'object') {
    return '';
  }
  return [
    policy.identity || '',
    policy.additiveContextRule || '',
    policy.unavailableEvidenceRule || '',
    policy.externalSearchPolicy?.modelRule || '',
  ].filter(Boolean).join(' ');
}

function buildMediaExtractionPolicy() {
  return {
    ...MEDIA_EXTRACTION_POLICY,
    supportedSources: [...MEDIA_EXTRACTION_POLICY.supportedSources],
    unsupportedSources: [...MEDIA_EXTRACTION_POLICY.unsupportedSources],
    modelRequestActions: [...MEDIA_EXTRACTION_POLICY.modelRequestActions],
    controlledPipeline: [...MEDIA_EXTRACTION_POLICY.controlledPipeline],
    outputArtifacts: [...MEDIA_EXTRACTION_POLICY.outputArtifacts],
  };
}

function formatMediaExtractionPolicyForModel(policy) {
  if (!policy || typeof policy !== 'object') {
    return '';
  }
  const supportedSources = Array.isArray(policy.supportedSources)
    ? policy.supportedSources.join('、')
    : '';
  const unsupportedSources = Array.isArray(policy.unsupportedSources)
    ? policy.unsupportedSources.join('、')
    : '';
  const modelRequestActions = Array.isArray(policy.modelRequestActions)
    ? policy.modelRequestActions.join('、')
    : '';
  const controlledPipeline = Array.isArray(policy.controlledPipeline)
    ? policy.controlledPipeline.join(' -> ')
    : '';
  const outputArtifacts = Array.isArray(policy.outputArtifacts)
    ? policy.outputArtifacts.slice(0, 16).join('、')
    : '';
  return [
    `媒体提取边界：支持 ${supportedSources || '无'}；不支持 ${unsupportedSources || '无'}。`,
    modelRequestActions ? `模型请求入口：${modelRequestActions}。` : '',
    controlledPipeline ? `后台受控链路：${controlledPipeline}。` : '',
    outputArtifacts ? `关键产物：${outputArtifacts}。` : '',
    policy.accessRule || '',
    policy.evidenceRule || '',
  ].filter(Boolean).join(' ');
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
  const parseCounts = datasets.reduce((counts, dataset) => {
    mergeCountSummary(counts, datasetParseStatusSummary(dataset), dataset?.lifecycle || dataset?.status || 'unknown');
    return counts;
  }, {});
  if (Object.keys(parseCounts).length) {
    return Object.entries(parseCounts)
      .map(([key, count]) => `${key}:${count}`)
      .join('，');
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

function mergeCountSummary(counts, summary, fallbackKey = 'unknown') {
  const text = String(summary || '').trim();
  if (!text || text === fallbackKey) return;
  let matched = false;
  text.split(/[，,;]/).forEach((part) => {
    const [rawKey, rawCount] = part.split(':');
    const key = String(rawKey || '').trim();
    const count = Number(String(rawCount || '').trim());
    if (key && Number.isFinite(count) && count > 0) {
      counts[key] = (counts[key] || 0) + count;
      matched = true;
    }
  });
  if (!matched && text) {
    counts[text] = (counts[text] || 0) + 1;
  }
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
    documentTitleHints: datasetDocumentTitleHints(dataset, { limit: 4 }),
    nounTermHints: datasetStringList(dataset, ['noun_term_hints', 'nounTermHints', 'noun_terms', 'nounTerms'], 6),
    sectionTitleHints: datasetStringList(dataset, ['section_title_hints', 'sectionTitleHints', 'document_section_hints', 'documentSectionHints'], 6),
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

function datasetStringList(dataset = {}, keys = [], limit = 6) {
  for (const key of keys) {
    const value = dataset[key];
    if (Array.isArray(value)) {
      return value
        .map((item) => String(item || '').trim())
        .filter(Boolean)
        .slice(0, limit);
    }
  }
  return [];
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
  const titleHints = Array.isArray(item.documentTitleHints) && item.documentTitleHints.length
    ? `/主题:${item.documentTitleHints.join('+')}`
    : '';
  const nounHints = Array.isArray(item.nounTermHints) && item.nounTermHints.length
    ? `/名词:${item.nounTermHints.join('+')}`
    : '';
  const sectionHints = Array.isArray(item.sectionTitleHints) && item.sectionTitleHints.length
    ? `/段落:${item.sectionTitleHints.join('+')}`
    : '';
  const parse = item.parseStatusSummary && item.parseStatusSummary !== item.lifecycle
    ? `/解析:${item.parseStatusSummary}`
    : '';
  return `${item.title}(${item.documentCount}文档/${item.lifecycle}${parse}${titleHints}${sectionHints}${nounHints}${hints})`;
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
  const previewStale = previewStatus === 'stale' || active?.imageJob?.status === 'stale';
  const dataQuality = summarizeStaticPageDataQuality(active);
  return {
    activeDraftId: active?.backendDraftId || active?.id || '',
    activeDraftStatus: active?.status || '',
    activeDraftObjective: String(active?.objective || active?.title || '').slice(0, 120),
    activeStyleDirection: active?.styleDirection || '',
    activeModuleCount: activeModules.length,
    activeEchartsModuleCount: echartsModules,
    activeDeterministicModuleCount: Math.max(0, activeModules.length - echartsModules),
    previewStatus,
    previewStale,
    finalRenderStatus: finalStatus,
    canEditModules: Boolean(active && activeModules.length),
    canExportFinal: finalStatus === 'rendered' && !previewStale,
    dataQuality,
    latestDrafts: draftList.slice(0, 5).map((draft) => ({
      id: draft?.backendDraftId || draft?.id || '',
      title: String(draft?.objective || draft?.title || '静态页草稿').slice(0, 80),
      status: draft?.previewContract?.status === 'stale' || draft?.imageJob?.status === 'stale'
        ? 'stale'
        : draft?.finalPage?.status || draft?.status || draft?.backendStatus || 'draft',
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
    if (workspace.previewStale) {
      parts.push('当前规划已变更，旧效果图和最终页不能继续复用，应先重新生成并确认效果图。');
    }
    if (workspace.canExportFinal) {
      parts.push('当前静态页已可导出 index.html 和 ZIP 交付包。');
    }
    const dataQualityText = formatStaticPageDataQualityForModel(workspace.dataQuality);
    if (dataQualityText) {
      parts.push(dataQualityText);
    }
  }
  if (workspace.latestDrafts?.length) {
    parts.push(`最近静态页：${workspace.latestDrafts.map((draft) => `${draft.title}(${draft.status})`).join('；')}`);
  }
  return `静态页工作区：${parts.join(' ')}`;
}

function summarizeStaticPageDataQuality(draft) {
  if (!draft || typeof draft !== 'object') {
    return null;
  }
  const snapshot = draft.dataSnapshot || draft.data_snapshot || {};
  const bindings = Array.isArray(snapshot.moduleBindings)
    ? snapshot.moduleBindings
    : Array.isArray(snapshot.module_bindings)
      ? snapshot.module_bindings
      : [];
  const moduleCount = Array.isArray(draft.modules) && draft.modules.length
    ? draft.modules.length
    : bindings.length;
  if (!moduleCount && !bindings.length) {
    return {
      status: 'not_available',
      moduleCount: 0,
      bindingCount: 0,
      attentionModuleCount: 0,
      readyModuleCount: 0,
      unknownBindingCount: 0,
      attentionModules: [],
      recommendedActions: [],
    };
  }
  if (!bindings.length) {
    return {
      status: 'unknown',
      moduleCount,
      bindingCount: 0,
      attentionModuleCount: 0,
      readyModuleCount: 0,
      unknownBindingCount: moduleCount,
      attentionModules: [],
      recommendedActions: ['static_page.update_draft'],
    };
  }

  const attentionModules = bindings
    .filter(staticPageBindingNeedsAttention)
    .map(staticPageBindingAttentionSummary)
    .slice(0, 5);
  const readyModuleCount = bindings.length - attentionModules.length;
  return {
    status: attentionModules.length ? 'attention_required' : 'ready',
    moduleCount,
    bindingCount: bindings.length,
    attentionModuleCount: attentionModules.length,
    readyModuleCount,
    unknownBindingCount: Math.max(0, moduleCount - bindings.length),
    attentionModules,
    recommendedActions: attentionModules.length
      ? ['retrieval.search', 'retrieval.read_detail', 'static_page.update_draft']
      : ['submit_static_page_image_preview'],
  };
}

function staticPageBindingNeedsAttention(binding = {}) {
  const status = staticPageBindingStatus(binding);
  if (status && !STATIC_PAGE_READY_BINDING_STATUSES.has(status)) return true;

  const chartDataFit = staticPageBindingChartDataFit(binding);
  if (chartDataFit && !STATIC_PAGE_READY_CHART_FITS.has(chartDataFit)) return true;

  const visualizationType = String(binding.visualizationType || binding.visualization_type || '').trim();
  return STATIC_PAGE_VISUALIZATIONS_REQUIRING_SAMPLE_ROWS.has(visualizationType)
    && staticPageBindingSampleRows(binding) === 0;
}

function staticPageBindingAttentionSummary(binding = {}) {
  return {
    moduleId: String(binding.moduleId || binding.module_id || binding.id || '').slice(0, 80),
    title: String(binding.title || binding.moduleTitle || binding.module_title || binding.moduleId || binding.module_id || '未命名模块').slice(0, 80),
    bindingQualityStatus: staticPageBindingStatus(binding) || 'unknown',
    chartDataFit: staticPageBindingChartDataFit(binding) || 'unknown',
    visualizationType: String(binding.visualizationType || binding.visualization_type || '').slice(0, 60),
    sampleRows: staticPageBindingSampleRows(binding),
    recommendedAction: String(
      binding.recommendedAction
        || binding.recommended_action
        || binding.bindingQuality?.recommendedAction
        || binding.binding_quality?.recommended_action
        || 'repair_module_data',
    ).slice(0, 80),
  };
}

function staticPageBindingStatus(binding = {}) {
  return String(
    binding.bindingQualityStatus
      || binding.binding_quality_status
      || binding.bindingQuality?.status
      || binding.binding_quality?.status
      || '',
  ).trim();
}

function staticPageBindingChartDataFit(binding = {}) {
  return String(
    binding.chartDataFit
      || binding.chart_data_fit
      || binding.bindingQuality?.chartDataFit
      || binding.binding_quality?.chart_data_fit
      || '',
  ).trim();
}

function staticPageBindingSampleRows(binding = {}) {
  const qualityRows = Number(binding.bindingQuality?.sampleRows ?? binding.binding_quality?.sample_rows);
  if (Number.isFinite(qualityRows)) return qualityRows;
  const rows = binding.sampleData || binding.sample_data;
  return Array.isArray(rows) ? rows.length : 0;
}

function formatStaticPageDataQualityForModel(quality) {
  if (!quality || typeof quality !== 'object') {
    return '';
  }
  if (quality.status === 'attention_required') {
    const modules = Array.isArray(quality.attentionModules)
      ? quality.attentionModules.slice(0, 3).map(formatStaticPageAttentionModule).filter(Boolean)
      : [];
    return [
      `静态页数据质量：${quality.attentionModuleCount || 0}/${quality.bindingCount || quality.moduleCount || 0} 个模块需要先补证据或修复绑定`,
      modules.length ? `重点模块：${modules.join('、')}` : '',
      '在提交效果图或最终渲染前，应优先让 V3 检索/细读/修复模块数据；没有 V3 供料时必须说明当前不可见/未供料，不能编造图表数据。',
    ].filter(Boolean).join('。');
  }
  if (quality.status === 'unknown') {
    return `静态页数据质量：当前有 ${quality.moduleCount || 0} 个模块，但没有模块绑定质量快照；提交效果图前应先刷新 dataSnapshot 或补齐模块数据。`;
  }
  if (quality.status === 'ready') {
    return `静态页数据质量：当前 ${quality.readyModuleCount || 0} 个模块绑定未发现阻塞，可继续效果图或最终渲染；仍需按 V3 已供料证据回答。`;
  }
  return '';
}

function formatStaticPageAttentionModule(module = {}) {
  const markers = [
    module.bindingQualityStatus && module.bindingQualityStatus !== 'unknown' ? module.bindingQualityStatus : '',
    module.chartDataFit && module.chartDataFit !== 'unknown' ? module.chartDataFit : '',
    Number.isFinite(Number(module.sampleRows)) ? `样本${Number(module.sampleRows)}` : '',
    module.recommendedAction ? `建议${module.recommendedAction}` : '',
  ].filter(Boolean);
  return `${module.title || module.moduleId || '未命名模块'}${markers.length ? `(${markers.join('/')})` : ''}`;
}
