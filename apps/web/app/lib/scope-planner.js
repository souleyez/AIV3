const MEDIA_DATASET_PATTERN = /音视频|音频|视频|录音|转写|字幕|会议|访谈|关键帧|ocr/i;

const DATASET_HINTS = [
  { pattern: /订单|销售|营收|收入|库存|发货|客单|转化|复购|经营/, label: '订单' },
  { pattern: /客服|工单|投诉|满意|售后|咨询|回复|评价/, label: '客服' },
  { pattern: /企业问答|制度|流程|员工|手册|政策|组织|公司介绍|FAQ|问答/i, label: '企业问答' },
  { pattern: /网页|采集|官网|竞品|新闻|页面|站点|爬取|抓取/, label: '网页采集' },
  { pattern: MEDIA_DATASET_PATTERN, label: '音视频' },
  { pattern: MEDIA_DATASET_PATTERN, label: '录音' },
  { pattern: MEDIA_DATASET_PATTERN, label: '视频' },
  { pattern: MEDIA_DATASET_PATTERN, label: '会议' },
];

const CONVERSATION_HINT = /刚才|上面|之前|继续|按你说的|这个|那版|草稿|修改|调整|确认|不要|改成|换成/;
const STATIC_PAGE_HINT = /静态页|静态页面|页面规划|一页|生成页面|落地页|模块|效果图|出图/;
const STATIC_PAGE_EDIT_HINT = /标题|文案|内容|数据|图表|布局|模块|调整|修改|改成|换成|突出|减少|增加|放大|缩小|移动|排序|风格|确认|效果图|导出/;
const REPORT_HINT = /报表|报告|周报|月报|经营分析|汇报|可视化|看板|dashboard/i;
const DATA_QUESTION_HINT = /分析|总结|趋势|原因|风险|机会|对比|明细|指标|数据|检索|查找|引用|音视频|音频|视频|录音|转写|字幕|会议|访谈|关键帧|ocr/i;

const INTENT_LABELS = {
  static_page: '静态页规划',
  report: '报表/看板',
  data_question: '资料问答',
  ordinary_chat: '普通聊天',
};

export function planAssistantScope({
  prompt = '',
  datasets = [],
  selectedDatasetId = '',
  conversationMemory = [],
  activeStaticPageDraft = null,
} = {}) {
  const normalizedPrompt = String(prompt || '').trim();
  const visibleDatasets = Array.isArray(datasets) ? datasets : [];
  const candidates = [];
  const staticDraftReference = buildStaticDraftReference(activeStaticPageDraft);
  const promptTouchesActiveStaticDraft = Boolean(
    staticDraftReference && (STATIC_PAGE_HINT.test(normalizedPrompt) || STATIC_PAGE_EDIT_HINT.test(normalizedPrompt)),
  );
  const intent = inferAssistantIntent(normalizedPrompt, {
    hasActiveStaticPageDraft: Boolean(staticDraftReference),
    promptTouchesActiveStaticDraft,
  });

  const selectedDataset = visibleDatasets.find((dataset) => dataset.id === selectedDatasetId);
  if (selectedDataset) {
    candidates.push(buildDatasetCandidate(selectedDataset, {
      confidence: 'high',
      reason: '用户当前已选中该供料范围',
      source: 'user_selected',
    }));
  }

  for (const dataset of visibleDatasets) {
    if (!dataset?.id || dataset.id === selectedDatasetId) continue;
    const haystack = `${dataset.title || ''} ${dataset.key || ''} ${dataset.description || ''}`;
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
  const datasetCandidate = (plan?.candidates || []).find((candidate) => candidate.type === 'dataset');
  return datasetCandidate?.id || '';
}

function textMatches(prompt, text) {
  const tokens = String(text || '')
    .split(/[\s,，。:：/\\|_-]+/)
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
  return {
    id,
    label: objective ? `当前静态页：${objective}` : '当前静态页草稿',
  };
}

function buildScopeHint(candidates, intent = 'ordinary_chat') {
  const visible = dedupeCandidates(candidates)
    .map(formatCandidateHint)
    .filter(Boolean)
    .slice(0, 3);
  const parts = [];
  if (visible.length) {
    parts.push(`可能相关：${visible.join('、')}`);
  }
  const intentLabel = INTENT_LABELS[intent] || '';
  if (intentLabel && intent !== 'ordinary_chat') {
    parts.push(`意图：${intentLabel}`);
  }
  return parts.join('；');
}

function formatCandidateHint(candidate) {
  if (!candidate?.label) return '';
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
  if (DATA_QUESTION_HINT.test(prompt) || DATASET_HINTS.some((hint) => hint.pattern.test(prompt))) {
    return 'data_question';
  }
  return 'ordinary_chat';
}

function buildSupplyStrategy(intent, candidates, prompt = '') {
  const hasDataset = candidates.some((candidate) => candidate.type === 'dataset');
  const hasStaticPageDraft = candidates.some((candidate) => candidate.type === 'static_page_draft');
  const needsDetail = hasDataset && (['static_page', 'report'].includes(intent) || MEDIA_DATASET_PATTERN.test(prompt));
  return {
    intent,
    answerPolicy: 'model_authored_host_supplied',
    currentArtifactPolicy: hasStaticPageDraft ? 'active_static_page_draft' : 'none',
    historyPolicy: candidates.some((candidate) => candidate.type === 'conversation_memory')
      ? 'intent_gated_selected'
      : 'intent_gated',
    retrievalPolicy: hasDataset ? (needsDetail ? 'detail_first' : 'standard') : 'not_requested',
    preferDetail: needsDetail,
    noFakeData: true,
  };
}
