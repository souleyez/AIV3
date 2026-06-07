const UPLOAD_DATASET_HINTS = [
  {
    label: '订单',
    key: 'orders',
    pattern: /订单|销售|营收|收入|库存|发货|客单|转化|复购|经营|order|sales|revenue|inventory|shipment/i,
  },
  {
    label: '客服',
    key: 'support',
    pattern: /客服|工单|投诉|满意|售后|咨询|回复|评价|support|ticket|complaint|after[-_\s]?sales/i,
  },
  {
    label: '企业问答',
    key: 'enterprise_faq',
    pattern: /企业问答|制度|流程|员工|手册|政策|组织|公司介绍|faq|handbook|policy|process/i,
  },
  {
    label: '网页采集',
    key: 'web_collection',
    pattern: /网页|采集|官网|竞品|新闻|页面|站点|爬取|抓取|crawl|website|web|site|news/i,
  },
];

const UNCLASSIFIED_DATASET = {
  label: '未分类',
  key: 'unclassified',
};

export function classifyUploadTarget({
  file = null,
  datasets = [],
  selectedDatasetId = '',
} = {}) {
  const visibleDatasets = Array.isArray(datasets) ? datasets : [];
  const selectedDataset = visibleDatasets.find((dataset) => dataset?.id === selectedDatasetId);
  if (selectedDataset) {
    return buildClassification({
      dataset: selectedDataset,
      confidence: 'high',
      reason: '当前已选中该数据集，上传优先进入当前供料范围。',
      source: 'user_selected',
    });
  }

  const fileSignal = `${file?.name || ''} ${file?.type || ''}`;
  const matchedHint = UPLOAD_DATASET_HINTS.find((hint) => hint.pattern.test(fileSignal));
  if (matchedHint) {
    const dataset = findDatasetForHint(visibleDatasets, matchedHint);
    return buildClassification({
      dataset,
      confidence: dataset ? 'medium' : 'low',
      reason: dataset
        ? `文件名或类型命中“${matchedHint.label}”主题，已匹配已有数据集。`
        : `文件名或类型命中“${matchedHint.label}”主题，将创建默认公开数据集。`,
      source: 'upload_classified',
      suggestedDataset: matchedHint,
    });
  }

  const fallbackDataset = findDatasetForHint(visibleDatasets, UNCLASSIFIED_DATASET);
  return buildClassification({
    dataset: fallbackDataset,
    confidence: fallbackDataset ? 'low' : 'none',
    reason: fallbackDataset
      ? '未命中明确业务主题，进入已有未分类数据集。'
      : '未命中明确业务主题，将创建默认公开未分类数据集。',
    source: 'fallback_unclassified',
    suggestedDataset: UNCLASSIFIED_DATASET,
  });
}

export function buildLocalUploadObjectKey(file, timestamp = Date.now()) {
  const rawName = file?.name || 'upload.bin';
  const safeName = rawName
    .normalize('NFKD')
    .replace(/[^\w.\-\u4e00-\u9fa5]+/g, '-')
    .replace(/-{2,}/g, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 120) || 'upload.bin';
  return `browser-uploads/${timestamp}-${safeName}`;
}

export function buildUploadDatasetPayload(classification) {
  const suggested = classification?.suggestedDataset || UNCLASSIFIED_DATASET;
  return {
    key: suggested.key || UNCLASSIFIED_DATASET.key,
    title: suggested.label || UNCLASSIFIED_DATASET.label,
    description: '默认公开数据集。由上传自动归类创建，未绑定私密密钥时所有用户可见。',
  };
}

export function inferUploadMediaKind(file = null) {
  const signal = `${file?.name || ''} ${file?.type || ''}`.toLowerCase();
  if (/(^|\W)video\//.test(signal) || /\.(mp4|mov|m4v|webm|mkv|avi|mpeg|mpg)(\W|$)/i.test(signal)) {
    return 'video';
  }
  if (/(^|\W)audio\//.test(signal) || /\.(mp3|wav|m4a|aac|flac|ogg|opus)(\W|$)/i.test(signal)) {
    return 'audio';
  }
  return '';
}

export function summarizeUploadClassification({ fileCount = 0, datasetTitle = '', classification = null } = {}) {
  const countLabel = fileCount > 1 ? `${fileCount} 个文件` : '1 个文件';
  const target = datasetTitle || classification?.dataset?.title || classification?.suggestedDataset?.label || '未分类';
  const reason = classification?.reason || '已按当前上下文选择数据集。';
  return `已上传登记 ${countLabel}，归入「${target}」。${reason}`;
}

export function isPublicUploadClassification(classification) {
  return classification?.source !== 'user_selected';
}

function buildClassification({
  dataset = null,
  confidence = 'low',
  reason = '',
  source = 'upload_classified',
  suggestedDataset = null,
}) {
  return {
    dataset,
    datasetId: dataset?.id || '',
    datasetTitle: dataset?.title || dataset?.key || '',
    confidence,
    reason,
    source,
    suggestedDataset,
  };
}

function findDatasetForHint(datasets, hint) {
  const targetLabel = normalizeText(hint.label);
  const targetKey = normalizeText(hint.key);
  return datasets.find((dataset) => {
    const haystack = normalizeText(`${dataset?.title || ''} ${dataset?.key || ''} ${dataset?.description || ''}`);
    return haystack.includes(targetLabel) || haystack.includes(targetKey);
  }) || null;
}

function normalizeText(value) {
  return String(value || '')
    .toLowerCase()
    .replace(/\s+/g, '');
}
