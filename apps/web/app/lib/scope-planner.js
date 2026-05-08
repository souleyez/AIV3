const DATASET_HINTS = [
  { pattern: /订单|销售|营收|收入|库存|发货|客单|转化|复购|经营/, label: '订单' },
  { pattern: /客服|工单|投诉|满意|售后|咨询|回复|评价/, label: '客服' },
  { pattern: /企业问答|制度|流程|员工|手册|政策|组织|公司介绍|FAQ|问答/i, label: '企业问答' },
  { pattern: /网页|采集|官网|竞品|新闻|页面|站点|爬取|抓取/, label: '网页采集' },
];

const CONVERSATION_HINT = /刚才|上面|之前|继续|按你说的|这个|那版|草稿|修改|调整|确认|不要|改成|换成/;
const STATIC_PAGE_HINT = /静态页|静态页面|页面规划|一页|生成页面|落地页|模块|效果图|出图/;
const REPORT_HINT = /报表|报告|周报|月报|经营分析|汇报|可视化|看板|dashboard/i;
const DATA_QUESTION_HINT = /分析|总结|趋势|原因|风险|机会|对比|明细|指标|数据|检索|查找|引用/;

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
} = {}) {
  const normalizedPrompt = String(prompt || '').trim();
  const visibleDatasets = Array.isArray(datasets) ? datasets : [];
  const candidates = [];
  const intent = inferAssistantIntent(normalizedPrompt);

  const selectedDataset = visibleDatasets.find((dataset) => dataset.id === selectedDatasetId);
  if (selectedDataset) {
    candidates.push({
      type: 'dataset',
      id: selectedDataset.id,
      label: selectedDataset.title || selectedDataset.key || '当前数据集',
      confidence: 'high',
      reason: '用户当前已选中该供料范围',
      source: 'user_selected',
    });
  }

  for (const dataset of visibleDatasets) {
    if (!dataset?.id || dataset.id === selectedDatasetId) continue;
    const haystack = `${dataset.title || ''} ${dataset.key || ''} ${dataset.description || ''}`;
    const matchedByDatasetName = haystack && textMatches(normalizedPrompt, haystack);
    const matchedByCommonHint = DATASET_HINTS.some((hint) => hint.pattern.test(normalizedPrompt) && haystack.includes(hint.label));
    if (matchedByDatasetName || matchedByCommonHint) {
      candidates.push({
        type: 'dataset',
        id: dataset.id,
        label: dataset.title || dataset.key || '相关数据集',
        confidence: matchedByDatasetName ? 'high' : 'medium',
        reason: matchedByDatasetName ? '用户提到数据集名称或关键字' : '用户问题命中常用业务主题',
        source: 'scope_planner',
      });
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

  return {
    candidates: dedupeCandidates(candidates).slice(0, 4),
    hint: buildScopeHint(candidates, intent),
    intent,
    intentLabel: INTENT_LABELS[intent] || INTENT_LABELS.ordinary_chat,
    supplyStrategy: buildSupplyStrategy(intent, candidates),
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

function dedupeCandidates(candidates) {
  const seen = new Set();
  return candidates.filter((candidate) => {
    const key = `${candidate.type}:${candidate.id}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function buildScopeHint(candidates, intent = 'ordinary_chat') {
  const visible = dedupeCandidates(candidates)
    .map((candidate) => candidate.label)
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

function inferAssistantIntent(prompt) {
  if (STATIC_PAGE_HINT.test(prompt)) return 'static_page';
  if (REPORT_HINT.test(prompt)) return 'report';
  if (DATA_QUESTION_HINT.test(prompt) || DATASET_HINTS.some((hint) => hint.pattern.test(prompt))) {
    return 'data_question';
  }
  return 'ordinary_chat';
}

function buildSupplyStrategy(intent, candidates) {
  const hasDataset = candidates.some((candidate) => candidate.type === 'dataset');
  const needsDetail = hasDataset && ['static_page', 'report'].includes(intent);
  return {
    intent,
    answerPolicy: 'model_authored_host_supplied',
    historyPolicy: candidates.some((candidate) => candidate.type === 'conversation_memory')
      ? 'intent_gated_selected'
      : 'intent_gated',
    retrievalPolicy: hasDataset ? (needsDetail ? 'detail_first' : 'standard') : 'not_requested',
    preferDetail: needsDetail,
    noFakeData: true,
  };
}
