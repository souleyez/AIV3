export function buildAssistantStartupBriefing({
  datasets = [],
  reportPlans = [],
  publishedReports = [],
  latestMessages = [],
  activityEvents = [],
  selectedDataset = null,
} = {}) {
  const visibleDatasets = Array.isArray(datasets) ? datasets : [];
  const reports = Array.isArray(reportPlans) ? reportPlans : [];
  const published = Array.isArray(publishedReports) ? publishedReports : [];
  const messages = Array.isArray(latestMessages) ? latestMessages : [];
  const events = Array.isArray(activityEvents) ? activityEvents : [];

  const latestActivity = [
    latestActivityEvent(events),
    latestDatasetActivity(visibleDatasets),
    latestMessageActivity(messages),
  ].filter(Boolean)[0] || '暂无最近上传、采集或对话摘要。';

  return {
    productTruth: '这是一个围绕数据集、文档、采集源、检索供料、报表和静态页输出的智能数据工作台。',
    visibleDatasetCount: visibleDatasets.length,
    visibleDocumentCount: sumNumericField(visibleDatasets, ['document_count', 'documentCount', 'documents']),
    estimatedWordCount: sumNumericField(visibleDatasets, ['estimated_word_count', 'estimatedWordCount', 'word_count']),
    reportPlanCount: reports.length,
    publishedReportCount: published.length,
    selectedScopeLabel: selectedDataset?.title || '',
    latestActivity,
    parseStateSummary: summarizeParseState(visibleDatasets),
    defaultPublicCategories: ['订单', '客服', '企业问答', '网页采集', '未分类'],
    capabilities: [
      'ordinary_chat',
      'scope_plan',
      'retrieve',
      'read_detail',
      'compare',
      'upload_classify',
      'report_plan',
      'static_page_plan',
      'render',
      'controlled_action',
    ],
  };
}

export function formatStartupBriefingForModel(briefing) {
  const source = briefing || {};
  const parts = [
    source.productTruth,
    `可见数据集 ${Number(source.visibleDatasetCount || 0)} 个，文档 ${Number(source.visibleDocumentCount || 0)} 份，估算字数 ${Number(source.estimatedWordCount || 0)}。`,
    `报表草稿 ${Number(source.reportPlanCount || 0)} 个，已发布 ${Number(source.publishedReportCount || 0)} 个。`,
    source.selectedScopeLabel ? `当前供料范围：${source.selectedScopeLabel}` : '当前未选数据集，可按普通模型聊天回答。',
    `最近状态：${source.latestActivity || '暂无。'}`,
    `解析状态：${source.parseStateSummary || '暂无解析状态。'}`,
  ];
  return parts.filter(Boolean).join('\n');
}

function sumNumericField(items, fieldNames) {
  return items.reduce((total, item) => {
    const value = fieldNames
      .map((fieldName) => Number(item?.[fieldName]))
      .find((candidate) => Number.isFinite(candidate) && candidate > 0);
    return total + (value || 0);
  }, 0);
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
