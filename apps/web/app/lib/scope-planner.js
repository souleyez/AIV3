import { attachVisibleDocumentsToDatasets, datasetDocumentTitleHints } from './dataset-document-hints.js';

const MEDIA_DATASET_PATTERN = /音视频|音频|视频|录音|转写|字幕|会议|访谈|关键帧|ocr/i;
const RESUME_DATASET_PATTERN = /简历|履历|候选人|求职|招聘|人才|面试|任职|工作经历|教育经历|项目经历|雇主|公司名|就职公司|resume|cv|candidate|recruit/i;
const RESUME_ENTITY_SCAN_PATTERN = /(?=.*(简历|履历|候选人|求职|招聘|人才|resume|cv|candidate))(?=.*(公司名|公司|企业|雇主|任职|就职|工作经历|经历|company|employer))(?=.*(多少|几个|哪些|列出|统计|汇总|分布|全部|所有|提到|公司名|count|list|all))/i;
const VIDEO_PPT_EXTRACTION_PATTERN = /((视频|mp4|mov|m4v|webm|公开视频|视频地址|视频链接|url|URL|上传).*(ppt|PPT|幻灯片|课件|原文|字幕|转写|讲稿|提取))|((ppt|PPT|幻灯片|课件|原文|字幕|转写|讲稿|提取).*(视频|mp4|mov|m4v|webm|公开视频|视频地址|视频链接|url|URL|上传))/i;
const DIRECT_VIDEO_SOURCE_PATTERN = /https?:\/\/\S+|\.(mp4|mov|m4v|webm)(\b|$)|公开视频|视频地址|视频链接|url|URL/i;

const DATASET_HINTS = [
  { pattern: /订单|销售|营收|收入|库存|发货|客单|转化|复购|经营/, label: '订单' },
  { pattern: /客服|工单|投诉|满意|售后|咨询|回复|评价/, label: '客服' },
  { pattern: /企业问答|制度|流程|员工|手册|政策|组织|公司介绍|FAQ|问答/i, label: '企业问答' },
  { pattern: RESUME_DATASET_PATTERN, label: '简历' },
  { pattern: RESUME_DATASET_PATTERN, label: '候选人' },
  { pattern: RESUME_DATASET_PATTERN, label: '招聘' },
  { pattern: RESUME_DATASET_PATTERN, label: '人才' },
  { pattern: /网页|采集|官网|竞品|新闻|页面|站点|爬取|抓取/, label: '网页采集' },
  { pattern: MEDIA_DATASET_PATTERN, label: '音视频' },
  { pattern: MEDIA_DATASET_PATTERN, label: '录音' },
  { pattern: MEDIA_DATASET_PATTERN, label: '视频' },
  { pattern: MEDIA_DATASET_PATTERN, label: '会议' },
];

const CONVERSATION_HINT = /刚才|上面|之前|继续|按你说的|这个|那版|草稿|修改|调整|确认|不要|改成|换成/;
const STATIC_PAGE_HINT = /静态页|静态页面|页面规划|一页|生成页面|落地页|模块|效果图|出图/;
const STATIC_PAGE_EDIT_HINT = /继续|接着|下一步|刚才|上面|之前|这个|那版|草稿|标题|文案|内容|数据|图表|布局|模块|调整|修改|改|换|突出|减少|增加|放大|缩小|移动|排序|风格|确认|效果图|导出/;
const REPORT_HINT = /报表|报告|周报|月报|经营分析|汇报|可视化|看板|dashboard/i;
const DATA_QUESTION_HINT = /分析|总结|趋势|原因|风险|机会|对比|明细|指标|数据|检索|查找|引用|音视频|音频|视频|录音|转写|字幕|会议|访谈|关键帧|ocr|简历|履历|候选人|招聘|人才|面试|任职|工作经历|公司名|就职公司|雇主|resume|cv|candidate/i;

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

const INTENT_LABELS = {
  static_page: '静态页规划',
  report: '报表/看板',
  data_question: '资料问答',
  ordinary_chat: '普通聊天',
};

export function planAssistantScope({
  prompt = '',
  datasets = [],
  documents = [],
  selectedDatasetId = '',
  selectedDatasetIds = [],
  conversationMemory = [],
  activeStaticPageDraft = null,
} = {}) {
  const normalizedPrompt = String(prompt || '').trim();
  const visibleDatasets = attachVisibleDocumentsToDatasets(datasets, documents);
  const userSelectedDatasetIds = normalizeDatasetIds([
    ...selectedDatasetIds,
    selectedDatasetId,
  ]);
  const candidates = [];
  const staticDraftReference = buildStaticDraftReference(activeStaticPageDraft);
  const promptTouchesActiveStaticDraft = Boolean(
    staticDraftReference && (STATIC_PAGE_HINT.test(normalizedPrompt) || STATIC_PAGE_EDIT_HINT.test(normalizedPrompt)),
  );
  const intent = inferAssistantIntent(normalizedPrompt, {
    hasActiveStaticPageDraft: Boolean(staticDraftReference),
    promptTouchesActiveStaticDraft,
  });

  const selectedDatasets = userSelectedDatasetIds
    .map((datasetId) => visibleDatasets.find((dataset) => dataset.id === datasetId))
    .filter(Boolean);
  for (const selectedDataset of selectedDatasets) {
    candidates.push(buildDatasetCandidate(selectedDataset, {
      confidence: 'high',
      reason: '用户当前已选中该供料范围',
      source: 'user_selected',
    }));
  }

  if (staticDraftReference?.dataQualityStatus === 'attention_required' && staticDraftReference.selectedDatasetId) {
    const draftDataset = visibleDatasets.find((dataset) => dataset.id === staticDraftReference.selectedDatasetId);
    if (draftDataset) {
      candidates.push(buildDatasetCandidate(draftDataset, {
        confidence: 'high',
        reason: '当前静态页数据绑定需要修复，沿用草稿供料范围',
        source: 'active_artifact_data_quality',
      }));
    }
  }

  for (const dataset of visibleDatasets) {
    if (!dataset?.id || userSelectedDatasetIds.includes(dataset.id)) continue;
    const documentTitleHints = datasetDocumentTitleHints(dataset).join(' ');
    const nounTermHints = datasetStringList(dataset, ['noun_term_hints', 'nounTermHints', 'noun_terms', 'nounTerms']).join(' ');
    const sectionTitleHints = datasetStringList(dataset, ['section_title_hints', 'sectionTitleHints', 'document_section_hints', 'documentSectionHints']).join(' ');
    const haystack = [
      dataset.title,
      dataset.key,
      dataset.description,
      dataset.category,
      dataset.default_category,
      documentTitleHints,
      nounTermHints,
      sectionTitleHints,
      dataset.content_type_summary,
      dataset.contentTypeSummary,
      dataset.parse_status_summary,
      dataset.parseStatusSummary,
    ].filter(Boolean).join(' ');
    const matchedByDatasetName = haystack && textMatches(normalizedPrompt, haystack);
    const matchedByCommonHint = DATASET_HINTS.some((hint) => hint.pattern.test(normalizedPrompt) && haystack.includes(hint.label));
    if (matchedByDatasetName || matchedByCommonHint) {
      candidates.push(buildDatasetCandidate(dataset, {
        confidence: matchedByDatasetName ? 'high' : 'medium',
        reason: matchedByDatasetName ? '用户提到数据集名称或关键字' : '用户问题命中常用业务主题',
        source: 'scope_planner',
      }));
    }
  }

  if (CONVERSATION_HINT.test(normalizedPrompt) && Array.isArray(conversationMemory) && conversationMemory.length) {
    candidates.push({
      type: 'conversation_memory',
      id: 'local-thread',
      label: '本轮对话历史',
      confidence: 'medium',
      reason: '用户引用了刚才或已有草稿内容',
      source: 'scope_planner',
    });
  }

  if (staticDraftReference && (intent === 'static_page' || promptTouchesActiveStaticDraft)) {
    candidates.push({
      type: 'static_page_draft',
      id: staticDraftReference.id,
      label: staticDraftReference.label,
      status: staticDraftReference.status,
      styleDirection: staticDraftReference.styleDirection,
      moduleCount: staticDraftReference.moduleCount,
      previewStatus: staticDraftReference.previewStatus,
      finalRenderStatus: staticDraftReference.finalRenderStatus,
      previewStale: staticDraftReference.previewStale,
      selectedDatasetId: staticDraftReference.selectedDatasetId,
      dataQualityStatus: staticDraftReference.dataQualityStatus,
      dataQualityAttentionCount: staticDraftReference.dataQualityAttentionCount,
      dataQualityBindingCount: staticDraftReference.dataQualityBindingCount,
      dataQualityAttentionModules: staticDraftReference.dataQualityAttentionModules,
      confidence: promptTouchesActiveStaticDraft ? 'high' : 'medium',
      reason: promptTouchesActiveStaticDraft ? '用户正在调整当前打开的静态页草稿' : '当前主区域打开了静态页草稿',
      source: 'active_artifact',
    });
  }

  return {
    candidates: dedupeCandidates(candidates).slice(0, 4),
    hint: buildScopeHint(candidates, intent),
    intent,
    intentLabel: INTENT_LABELS[intent] || INTENT_LABELS.ordinary_chat,
    supplyStrategy: buildSupplyStrategy(intent, candidates, normalizedPrompt),
  };
}

export function selectPlannerDatasetId(plan) {
  return selectPlannerDatasetIds(plan)[0] || '';
}

export function selectPlannerDatasetIds(plan) {
  return normalizeDatasetIds(
    (plan?.candidates || [])
      .filter((candidate) => candidate.type === 'dataset')
      .map((candidate) => candidate.id),
  );
}

function normalizeDatasetIds(ids) {
  return [...new Set((Array.isArray(ids) ? ids : [ids])
    .map((id) => String(id || '').trim())
    .filter(Boolean))];
}

function textMatches(prompt, text) {
  const tokens = String(text || '')
    .split(/[\s,，。:：/\\|_\-.()（）]+/)
    .map((token) => token.trim())
    .filter((token) => token.length >= 2);
  return tokens.some((token) => prompt.includes(token));
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
  if (MEDIA_DATASET_PATTERN.test(haystack)) {
    hints.add('audio_video');
    hints.add('transcript_possible');
    hints.add('keyframe_ocr_possible');
  }
  return [...hints].filter((hint) => typeof hint === 'string' && hint.trim()).slice(0, 6);
}

function datasetStringList(dataset = {}, keys = [], limit = 8) {
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

function buildDatasetCandidate(dataset, { confidence, reason, source }) {
  return {
    type: 'dataset',
    id: dataset.id,
    label: dataset.title || dataset.key || '相关数据集',
    key: dataset.key || '',
    visibility: dataset.visibility || dataset.access || '',
    category: dataset.category || dataset.default_category || '',
    lifecycle: dataset.lifecycle || dataset.status || 'unknown',
    documentCount: datasetDocumentCount(dataset),
    estimatedWordCount: datasetEstimatedWordCount(dataset),
    parseStatusSummary: datasetParseStatusSummary(dataset),
    latestActivity: datasetLatestActivity(dataset),
    confidence,
    reason,
    source,
    materialHints: datasetMaterialHints(dataset),
    nounTermHints: datasetStringList(dataset, ['noun_term_hints', 'nounTermHints', 'noun_terms', 'nounTerms'], 8),
    sectionTitleHints: datasetStringList(dataset, ['section_title_hints', 'sectionTitleHints', 'document_section_hints', 'documentSectionHints'], 8),
  };
}

function datasetDocumentCount(dataset = {}) {
  const raw = dataset.document_count ?? dataset.documentCount ?? dataset.documents_count ?? dataset.documentsCount;
  if (Number.isFinite(Number(raw))) {
    return Number(raw);
  }
  return Array.isArray(dataset.documents) ? dataset.documents.length : 0;
}

function datasetEstimatedWordCount(dataset = {}) {
  const raw = dataset.estimated_word_count ?? dataset.estimatedWordCount ?? dataset.word_count ?? dataset.wordCount;
  return Number.isFinite(Number(raw)) ? Number(raw) : 0;
}

function datasetLatestActivity(dataset = {}) {
  const value = dataset.latestUpload || dataset.latest_upload || dataset.updated_at || dataset.updatedAt || dataset.created_at || dataset.createdAt || '';
  return typeof value === 'string' ? value.slice(0, 80) : '';
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

function dedupeCandidates(candidates) {
  const seen = new Set();
  return candidates.filter((candidate) => {
    const key = `${candidate.type}:${candidate.id}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function buildStaticDraftReference(draft) {
  if (!draft || typeof draft !== 'object') return null;
  const id = draft.backendDraftId || draft.backendId || draft.id || '';
  if (!id) return null;
  const objective = String(draft.objective || draft.title || '').trim();
  const previewStatus = draft.previewContract?.status || draft.imageJob?.status || '';
  const finalRenderStatus = draft.finalPage?.status || '';
  const dataQuality = summarizeStaticDraftDataQuality(draft);
  return {
    id,
    label: objective ? `当前静态页：${objective}` : '当前静态页草稿',
    status: draft.status || draft.backendStatus || '',
    styleDirection: draft.styleDirection || '',
    moduleCount: Array.isArray(draft.modules) ? draft.modules.length : 0,
    previewStatus,
    finalRenderStatus,
    previewStale: previewStatus === 'stale' || draft.imageJob?.status === 'stale',
    selectedDatasetId: draft.dataSnapshot?.selectedDatasetId || draft.data_snapshot?.selected_dataset_id || draft.datasetId || '',
    dataQualityStatus: dataQuality.status,
    dataQualityAttentionCount: dataQuality.attentionModuleCount,
    dataQualityBindingCount: dataQuality.bindingCount,
    dataQualityAttentionModules: dataQuality.attentionModules,
  };
}

function buildScopeHint(candidates, intent = 'ordinary_chat') {
  const visible = dedupeCandidates(candidates)
    .map(formatCandidateHint)
    .filter(Boolean)
    .slice(0, 3);
  const parts = [];
  if (visible.length) {
    parts.push(`已选中：${visible.join('、')}`);
  }
  const intentLabel = INTENT_LABELS[intent] || '';
  if (intentLabel && intent !== 'ordinary_chat') {
    parts.push(`意图：${intentLabel}`);
  }
  return parts.join('；');
}

function formatCandidateHint(candidate) {
  if (!candidate?.label) return '';
  if (candidate.type === 'static_page_draft') {
    const status = candidate.previewStale
      ? '规划已变更'
      : candidate.finalRenderStatus || candidate.previewStatus || candidate.status || '';
    const details = [];
    if (status) details.push(status);
    if (candidate.dataQualityStatus === 'attention_required') {
      details.push(`需修复数据${candidate.dataQualityAttentionCount || 0}`);
    }
    return details.length ? `${candidate.label}(${details.join('/')})` : candidate.label;
  }
  if (candidate.type !== 'dataset') return candidate.label;
  const details = [];
  if (candidate.documentCount) details.push(`${candidate.documentCount}文档`);
  if (candidate.materialHints?.includes('audio_video')) details.push('媒体');
  return details.length ? `${candidate.label}(${details.join('/')})` : candidate.label;
}

function inferAssistantIntent(prompt, options = {}) {
  if (STATIC_PAGE_HINT.test(prompt)) return 'static_page';
  if (REPORT_HINT.test(prompt)) return 'report';
  if (options.hasActiveStaticPageDraft && options.promptTouchesActiveStaticDraft) return 'static_page';
  if (VIDEO_PPT_EXTRACTION_PATTERN.test(prompt)) return 'data_question';
  if (DATA_QUESTION_HINT.test(prompt) || DATASET_HINTS.some((hint) => hint.pattern.test(prompt))) {
    return 'data_question';
  }
  return 'ordinary_chat';
}

function buildSupplyStrategy(intent, candidates, prompt = '') {
  const hasDataset = candidates.some((candidate) => candidate.type === 'dataset');
  const hasStaticPageDraft = candidates.some((candidate) => candidate.type === 'static_page_draft');
  const hasConversationMemory = candidates.some((candidate) => candidate.type === 'conversation_memory');
  const staticPageNeedsDataRepair = candidates.some((candidate) => (
    candidate.type === 'static_page_draft' && candidate.dataQualityStatus === 'attention_required'
  ));
  const staticPageMissingBindingSnapshot = candidates.some((candidate) => (
    candidate.type === 'static_page_draft' && candidate.dataQualityStatus === 'unknown'
  ));
  const wantsVideoPptExtraction = VIDEO_PPT_EXTRACTION_PATTERN.test(prompt);
  const wantsResumeEntityScan = RESUME_ENTITY_SCAN_PATTERN.test(prompt);
  const needsDetail = hasDataset && (
    ['static_page', 'report'].includes(intent)
    || MEDIA_DATASET_PATTERN.test(prompt)
    || wantsVideoPptExtraction
    || wantsResumeEntityScan
    || staticPageNeedsDataRepair
  );
  return {
    intent,
    answerPolicy: 'model_authored_host_supplied',
    currentArtifactPolicy: hasStaticPageDraft ? 'active_static_page_draft' : 'none',
    artifactDataQualityPolicy: staticPageNeedsDataRepair
      ? 'repair_before_preview_or_render'
      : staticPageMissingBindingSnapshot
        ? 'refresh_binding_snapshot'
        : hasStaticPageDraft
          ? 'respect_current_binding_quality'
          : 'none',
    actionPolicy: 'model_may_request_controlled_actions_host_validates',
    contextBudgetPolicy: needsDetail || hasConversationMemory
      ? 'quality_first_token_tolerant'
      : 'compact_until_retrieval_needed',
    candidatePolicy: hasDataset
      ? 'selected_or_inferred_visible_datasets_only'
      : 'ordinary_chat_without_forced_dataset',
    historyPolicy: hasConversationMemory
      ? 'intent_gated_selected'
      : 'intent_gated',
    retrievalPolicy: hasDataset ? (needsDetail ? 'detail_first' : 'standard') : 'not_requested',
    coveragePolicy: hasDataset && wantsResumeEntityScan ? 'document_entity_scan' : 'ranked_retrieval',
    preferDetail: needsDetail,
    recommendedActions: buildRecommendedActions(intent, {
      hasDataset,
      hasStaticPageDraft,
      staticPageNeedsDataRepair,
      wantsVideoPptExtraction,
      wantsResumeEntityScan,
      prompt,
    }),
    noFakeData: true,
  };
}

function buildRecommendedActions(intent, { hasDataset, hasStaticPageDraft, staticPageNeedsDataRepair, wantsVideoPptExtraction, wantsResumeEntityScan, prompt }) {
  const actions = [];
  if (hasDataset) {
    actions.push('retrieval.search');
  }
  if (hasDataset && (intent === 'static_page' || intent === 'report' || MEDIA_DATASET_PATTERN.test(prompt) || staticPageNeedsDataRepair || wantsResumeEntityScan)) {
    actions.push('retrieval.read_detail');
  }
  if (hasDataset && wantsResumeEntityScan) {
    actions.push('retrieval.scan_documents');
  }
  if (MEDIA_DATASET_PATTERN.test(prompt) && (!wantsVideoPptExtraction || hasDataset)) {
    actions.push('media.detail');
  }
  if (wantsVideoPptExtraction) {
    if (DIRECT_VIDEO_SOURCE_PATTERN.test(prompt)) {
      actions.push('media.resolve_video_url');
    }
    actions.push('media.extract_ppt_transcript');
  }
  if (intent === 'static_page') {
    actions.push(hasStaticPageDraft ? 'static_page.update_draft' : 'static_page.plan');
  }
  if (intent === 'report') {
    actions.push('report.plan');
  }
  if (!actions.length) {
    actions.push('ordinary_chat.answer');
  }
  return actions.slice(0, 5);
}

function summarizeStaticDraftDataQuality(draft) {
  const snapshot = draft?.dataSnapshot || draft?.data_snapshot || {};
  const bindings = Array.isArray(snapshot.moduleBindings)
    ? snapshot.moduleBindings
    : Array.isArray(snapshot.module_bindings)
      ? snapshot.module_bindings
      : [];
  if (!bindings.length) {
    return {
      status: Array.isArray(draft?.modules) && draft.modules.length ? 'unknown' : 'not_available',
      attentionModuleCount: 0,
      bindingCount: 0,
      attentionModules: [],
    };
  }
  const attentionModules = bindings
    .filter(staticPageBindingNeedsAttention)
    .map(staticPageBindingAttentionSummary)
    .slice(0, 5);
  return {
    status: attentionModules.length ? 'attention_required' : 'ready',
    attentionModuleCount: attentionModules.length,
    bindingCount: bindings.length,
    attentionModules,
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
    sampleRows: staticPageBindingSampleRows(binding),
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
