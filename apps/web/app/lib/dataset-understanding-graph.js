import {
  canonicalDocumentTitle,
  projectFallbackLabel,
} from './dataset-understanding-label-quality.js';
import { applyDatasetUnderstandingGraphBudget } from './dataset-understanding-graph-budget.js';

export const DATASET_GRAPH_CATEGORIES = [
  { key: 'dataset', name: '数据集', color: '#f8fbff' },
  { key: 'object', name: '业务对象', color: '#38bdf8' },
  { key: 'field', name: '关键字段', color: '#5eead4' },
  { key: 'concept', name: '共享概念', color: '#fbbf24' },
  { key: 'structure', name: '数据结构', color: '#93c5fd' },
  { key: 'unresolved', name: '待解释', color: '#94a3b8' },
  { key: 'document', name: '原始文档', color: '#5eead4' },
  { key: 'knowledge', name: '知识词', color: '#fbbf24' },
  { key: 'section', name: '章节结构', color: '#93c5fd' },
  { key: 'material', name: '资料类型', color: '#c4b5fd' },
  { key: 'strategy', name: '理解策略', color: '#fb7185' },
];

const LIMITS = {
  document: 18,
  knowledge: 20,
  section: 10,
  material: 8,
  strategy: 6,
};

function cleanText(value) {
  return typeof value === 'string' ? value.trim() : '';
}

function cleanList(value, limit = Number.POSITIVE_INFINITY) {
  const source = Array.isArray(value) ? value : value === null || value === undefined ? [] : [value];
  const seen = new Set();
  const result = [];
  const visit = (item) => {
    if (Array.isArray(item)) {
      item.forEach(visit);
      return;
    }
    const text = cleanText(item);
    const key = text.toLocaleLowerCase();
    if (!text || seen.has(key) || result.length >= limit) return;
    seen.add(key);
    result.push(text);
  };
  source.forEach(visit);
  return result;
}

function numericValue(value) {
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? number : 0;
}

function datasetDocumentIds(document) {
  return [...new Set([
    document?.dataset_id,
    document?.datasetId,
    ...(Array.isArray(document?.dataset_ids) ? document.dataset_ids : []),
    ...(Array.isArray(document?.datasetIds) ? document.datasetIds : []),
  ].map((value) => cleanText(String(value || ''))).filter(Boolean))];
}

function parseSummaryCount(summary, signals) {
  const source = cleanText(summary).toLocaleLowerCase();
  if (!source) return 0;
  for (const signal of signals) {
    const match = source.match(new RegExp(`${signal}\\s*[:=]\\s*(\\d+)`, 'i'));
    if (match) return Number(match[1]) || 0;
  }
  return 0;
}

function documentParseStatus(document) {
  return cleanText(document?.parse_status || document?.parseStatus).toLocaleLowerCase();
}

function documentLifecycle(document) {
  return cleanText(document?.lifecycle).toLocaleLowerCase();
}

function documentQualityStatus(document) {
  return cleanText(document?.parse_quality_status || document?.parseQualityStatus).toLocaleLowerCase();
}

function parsedDocument(document) {
  const status = documentParseStatus(document);
  const lifecycle = documentLifecycle(document);
  return ['completed', 'complete', 'parsed', 'indexed', 'ready', 'succeeded', 'success']
    .some((signal) => status.includes(signal) || lifecycle.includes(signal));
}

function readyDocument(document) {
  const status = documentParseStatus(document);
  const lifecycle = documentLifecycle(document);
  return lifecycle.includes('indexed') || status === 'indexed' || status === 'ready';
}

function attentionDocument(document) {
  const combined = `${documentParseStatus(document)} ${documentQualityStatus(document)}`;
  return ['attention', 'failed', 'failure', 'warning', 'error'].some((signal) => combined.includes(signal));
}

function knowledgeSignal(value) {
  return /\p{Script=Han}/u.test(cleanText(value)) ? 'concept' : 'identifier';
}

function readableStatus(value, fallback = '状态未返回') {
  const status = cleanText(value).toLocaleLowerCase();
  const labels = {
    completed: '已完成',
    complete: '已完成',
    parsed: '已解析',
    indexed: '可检索',
    ready: '可检索',
    received: '已接收',
    pending: '处理中',
    attention_required: '需要关注',
    ok: '质量正常',
  };
  return labels[status] || cleanText(value) || fallback;
}

function documentTypeLabel(contentType) {
  const value = cleanText(contentType).toLocaleLowerCase();
  if (!value) return '';
  if (value.includes('pdf')) return '文档资料';
  if (value.includes('sheet') || value.includes('excel') || value.includes('csv') || /(^|\W)xlsx?(\W|$)/.test(value)) return '表格';
  if (value.includes('zip') || value.includes('archive') || value.includes('compressed')) return '压缩资料';
  if (value.includes('word') || value.includes('document') || /(^|\W)docx?(\W|$)/.test(value)) return '文字文档';
  if (value.includes('presentation') || value.includes('powerpoint') || /(^|\W)pptx?(\W|$)/.test(value)) return '演示文稿';
  if (value.includes('markdown') || /(^|\W)md(\W|$)/.test(value)) return '标记文档';
  if (value.includes('html') || value.includes('web') || value.includes('url')) return '网页';
  if (value.startsWith('image/')) return '图片';
  if (value.startsWith('audio/') || value.startsWith('video/')) return '音视频';
  if (value.startsWith('text/')) return '文本';
  return contentType;
}

function projectedFallbackItems(value, kind, limit) {
  const allowed = [];
  const hidden = [];
  const seen = new Set();
  let duplicateCount = 0;
  cleanList(value).forEach((rawLabel, index) => {
    const projection = projectFallbackLabel(rawLabel, { kind });
    const item = {
      id: `${kind}:${index}`,
      rawLabel,
      ...projection,
    };
    if (!projection.main_canvas_allowed) {
      hidden.push({
        id: `hidden:${kind}:${index}`,
        kind,
        label: projection.display_label,
        qualityClass: projection.quality_class,
        reason: projection.reason,
        detail: projection.detail_label,
      });
      return;
    }
    const key = projection.normalized_key || projection.display_label.toLocaleLowerCase();
    if (seen.has(key)) {
      duplicateCount += 1;
      return;
    }
    seen.add(key);
    if (allowed.length < limit) allowed.push(item);
  });
  return { allowed, hidden, duplicateCount };
}

function summaryItems(value) {
  return cleanList(
    cleanText(value)
      .split(/[,;|，；]+/)
      .map((item) => cleanText(item).replace(/\s*[:=]\s*\d+\s*$/, ''))
      .map((item) => documentTypeLabel(item) || item),
    LIMITS.material,
  );
}

function normalizedLabel(value) {
  return cleanText(value)
    .toLocaleLowerCase()
    .replace(/\.[a-z0-9]{1,8}$/i, '')
    .replace(/[（(][^）)]*[）)]/g, ' ')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();
}

function labelTokens(value) {
  const normalized = normalizedLabel(value);
  const tokens = new Set();
  normalized.match(/[a-z0-9]{2,}|[\p{Script=Han}]+/gu)?.forEach((part) => {
    if (/^[a-z0-9]+$/u.test(part)) {
      tokens.add(part);
      return;
    }
    if (part.length <= 2) {
      tokens.add(part);
      return;
    }
    for (let index = 0; index < part.length - 1; index += 1) {
      tokens.add(part.slice(index, index + 2));
    }
  });
  return tokens;
}

function labelAffinity(left, right) {
  const leftTokens = labelTokens(left);
  const rightTokens = labelTokens(right);
  const sharedTokens = [...leftTokens].filter((token) => rightTokens.has(token));
  const unionSize = new Set([...leftTokens, ...rightTokens]).size;
  return {
    score: unionSize ? sharedTokens.length / unionSize : 0,
    sharedTokens,
  };
}

function categoryIndex(key) {
  return Math.max(0, DATASET_GRAPH_CATEGORIES.findIndex((category) => category.key === key));
}

function nodeId(kind, value, index = 0) {
  const normalized = cleanText(value)
    .toLocaleLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^\p{L}\p{N}_.:-]+/gu, '-')
    .slice(0, 64);
  return `${kind}:${normalized || index}`;
}

function nodeShortLabel(value) {
  const normalized = cleanText(value)
    .replace(/\.[a-z0-9]{1,8}$/i, '')
    .replace(/\s+/g, '');
  return Array.from(normalized || '节点').slice(0, 5).join('');
}

const FIELD_ROLE_SUFFIXES = {
  identifier: '标识',
  name: '名称',
  date: '日期',
  amount: '金额',
  quantity: '数量',
  category: '分类',
  status: '状态',
  location: '位置',
  text: '要点',
  unknown: '字段',
};

function chineseLabelText(value) {
  return (cleanText(value)
    .replace(/[A-Za-z][A-Za-z0-9_.-]*/g, ' ')
    .replace(/\d+(?:[./_-]\d+)*/g, ' ')
    .match(/\p{Script=Han}+/gu) || [])
    .join('');
}

function conciseChineseLabel(value, maximum = 10) {
  const chinese = chineseLabelText(value);
  return Array.from(chinese).length >= 2 && Array.from(chinese).length <= maximum
    ? chinese
    : '';
}

function objectKindDisplayLabel(kind) {
  return {
    database: '业务数据表',
    database_table: '业务数据表',
    spreadsheet: '表格数据',
    spreadsheet_table: '表格数据',
    document: '业务资料',
    document_section: '文档结构',
    asset: '资料资产',
    asset_profile: '资料资产',
    media: '媒体内容',
    media_segment: '媒体内容',
    web_api: '接口数据',
    api_resource: '接口数据',
  }[cleanText(kind).toLocaleLowerCase()] || '业务对象';
}

function semanticObjectDisplayLabel(object) {
  if (object?.status === 'unresolved') return '待解释对象';
  return conciseChineseLabel(object?.label) || objectKindDisplayLabel(object?.kind);
}

function objectLabelAnchor(value) {
  const characters = Array.from(chineseLabelText(value));
  if (!characters.length) return '业务';
  return characters.slice(-Math.min(2, characters.length)).join('');
}

function semanticFieldDisplayLabel(field, parentLabel) {
  const anchor = objectLabelAnchor(parentLabel);
  if (field?.status === 'unresolved') return `${anchor}待解释`;
  const rawLabel = cleanText(field?.label);
  const containsEnglishIdentifier = /[A-Za-z][A-Za-z0-9_.-]*/.test(rawLabel);
  const directChineseLabel = containsEnglishIdentifier ? '' : conciseChineseLabel(rawLabel, 8);
  if (directChineseLabel) return directChineseLabel;
  const suffix = FIELD_ROLE_SUFFIXES[cleanText(field?.semantic_role).toLocaleLowerCase()] || '字段';
  return anchor.endsWith(suffix) ? anchor : `${anchor}${suffix}`;
}

function trustedBusinessFieldLabel(field) {
  const rawLabel = cleanText(field?.label);
  return field?.status !== 'unresolved'
    && !/[A-Za-z][A-Za-z0-9_.-]*/.test(rawLabel)
    && Boolean(conciseChineseLabel(rawLabel, 8));
}

function fieldBusinessScore(field) {
  const labelSource = cleanText(field?.label_source).toLocaleLowerCase();
  const role = cleanText(field?.semantic_role).toLocaleLowerCase();
  const labelLength = Array.from(chineseLabelText(field?.label)).length;
  return (labelSource.includes('confirmed') || labelSource.includes('dictionary') ? 80 : 0)
    + (labelSource.includes('comment') ? 55 : 0)
    + (role && role !== 'unknown' ? 30 : 0)
    + Math.round((Number(field?.confidence) || 0) * 20)
    + Math.min(18, Math.log2(Math.max(0, Number(field?.non_empty_count) || 0) + 1) * 2)
    + (labelLength >= 2 && labelLength <= 6 ? 15 : 0);
}

function graphNode({ id, name, kind, detail, evidence, status = '', signal = '', symbolSize = 30 }) {
  return {
    id,
    name,
    shortLabel: nodeShortLabel(name),
    kind,
    categoryKey: kind,
    category: categoryIndex(kind),
    detail,
    evidence,
    status,
    signal,
    symbolSize,
    value: 1,
  };
}

function selectionModel() {
  return {
    hasDataset: false,
    mode: 'empty',
    viewLabel: '',
    overviewTitle: '',
    snapshotStatus: 'idle',
    statusMessage: '',
    datasetId: '',
    title: '',
    categories: DATASET_GRAPH_CATEGORIES,
    metrics: {
      documentCount: 0,
      estimatedWordCount: 0,
      parsedDocumentCount: 0,
      readyDocumentCount: 0,
      attentionDocumentCount: 0,
      knowledgeCount: 0,
      observedRelationCount: 0,
      inferredRelationCount: 0,
      confirmedRelationCount: 0,
      crossNodeRelationCount: 0,
      sourceCount: 0,
      objectCount: 0,
      fieldCount: 0,
      unresolvedFieldCount: 0,
    },
    pipeline: [],
    understanding: {
      summary: '',
      keyConcepts: [],
      keyFields: [],
      technicalIdentifiers: [],
      structurePath: [],
      strategies: [],
      retrievalCoverage: { ready: 0, total: 0 },
    },
    nodes: [],
    links: [],
    emptyKnowledgeMessage: '',
    fallbackQuality: {
      hiddenCount: 0,
      hiddenItems: [],
      hiddenByClass: {},
      deduplicatedDocumentTitles: 0,
    },
  };
}

function semanticEvidenceText(references, fallback = '') {
  const labels = cleanList((Array.isArray(references) ? references : []).map((reference) => reference?.label), 3);
  return labels.join(' · ') || fallback || '语义快照';
}

function semanticStatusLabel(status) {
  return {
    confirmed: '已确认',
    observed: '已观察',
    inferred: '推断',
    unresolved: '待解释',
  }[status] || status || '状态未返回';
}

function semanticNodeSize(count, minimum, maximum) {
  const value = Math.max(0, Number(count) || 0);
  return Math.min(maximum, minimum + Math.log2(value + 1) * 3.4);
}

function withSemanticNeighborhoods(nodes, links) {
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  nodes.forEach((node) => {
    node.incoming = [];
    node.outgoing = [];
  });
  links.forEach((link) => {
    const source = nodeById.get(link.source);
    const target = nodeById.get(link.target);
    if (!source || !target) return;
    source.outgoing.push({ linkId: link.id, nodeId: target.id, relation: link.relation, type: link.type });
    target.incoming.push({ linkId: link.id, nodeId: source.id, relation: link.relation, type: link.type });
  });
  nodes.forEach((node) => {
    node.incoming.sort((left, right) => left.nodeId.localeCompare(right.nodeId));
    node.outgoing.sort((left, right) => left.nodeId.localeCompare(right.nodeId));
  });
  return nodes;
}

export function graphNeighborhoodIds(model, focusNodeId, depth = 1) {
  const validNodeIds = new Set((Array.isArray(model?.nodes) ? model.nodes : []).map((node) => node.id));
  const focusId = cleanText(focusNodeId);
  if (!focusId || !validNodeIds.has(focusId)) return new Set();
  const boundedDepth = Math.max(0, Math.min(2, Number(depth) || 0));
  const adjacency = new Map([...validNodeIds].map((id) => [id, new Set()]));
  (Array.isArray(model?.links) ? model.links : []).forEach((link) => {
    if (!validNodeIds.has(link.source) || !validNodeIds.has(link.target)) return;
    adjacency.get(link.source).add(link.target);
    adjacency.get(link.target).add(link.source);
  });
  const visible = new Set([focusId]);
  let frontier = new Set([focusId]);
  for (let level = 0; level < boundedDepth; level += 1) {
    const next = new Set();
    frontier.forEach((id) => adjacency.get(id)?.forEach((neighbor) => {
      if (!visible.has(neighbor)) next.add(neighbor);
      visible.add(neighbor);
    }));
    frontier = next;
  }
  return visible;
}

export function filterDatasetUnderstandingGraph(model, options = {}) {
  const {
    activeCategory = 'all',
    activeRelationType = 'all',
    viewMode = 'business',
    focusNodeId = '',
    focusDepth = 'all',
    focusDatasetId = '',
    density = 'standard',
  } = options;
  const localIds = focusDepth === 'all'
    ? null
    : graphNeighborhoodIds(model, focusNodeId, focusDepth);
  const revealUnresolved = activeCategory === 'unresolved';
  const candidateNodes = model.nodes.filter((node) => {
    if (
      model.mode === 'cross'
      && focusDatasetId
      && !(Array.isArray(node.datasetRefs) && node.datasetRefs.includes(focusDatasetId))
    ) return false;
    if (activeCategory !== 'all' && node.kind !== 'dataset' && node.kind !== activeCategory) return false;
    if (
      model.mode === 'semantic'
      && viewMode === 'business'
      && !revealUnresolved
      && node.kind !== 'dataset'
      && (node.kind === 'unresolved' || node.technicalOnly)
    ) return false;
    if (localIds && !localIds.has(node.id)) return false;
    return true;
  });
  const candidateIds = new Set(candidateNodes.map((node) => node.id));
  const candidateLinks = model.links.filter((link) => (
    candidateIds.has(link.source) && candidateIds.has(link.target)
  ));
  const budgeted = applyDatasetUnderstandingGraphBudget({
    nodes: candidateNodes,
    links: candidateLinks,
  }, {
    density,
    selectedNodeId: focusNodeId,
    allowTechnical: viewMode !== 'business' || revealUnresolved,
  });
  if (activeRelationType === 'all') return budgeted;
  const links = budgeted.links.filter((link) => link.type === activeRelationType);
  return {
    ...budgeted,
    links,
    stats: {
      ...budgeted.stats,
      visibleEdgeCount: links.length,
    },
  };
}

const CROSS_DATASET_CLUSTER_COLORS = [
  '#38bdf8',
  '#a78bfa',
  '#fb7185',
  '#fbbf24',
  '#34d399',
  '#60a5fa',
  '#f97316',
  '#22d3ee',
];

const CROSS_SHARED_COLOR = '#f8fafc';
const CROSS_SHARED_KINDS = new Set(['document', 'field', 'concept']);

function safeSimilarityRelationLabel(value) {
  const label = cleanText(value);
  if (!label || /同一|真实共享|完全相同|确认为/u.test(label)) return '相似线索';
  return label;
}

function safeSimilarityReason(value) {
  const reason = cleanText(value);
  if (!reason || /同一|真实共享/u.test(reason)) {
    return '可见证据只提示语义相近，尚未形成确定身份或确定共享关系。';
  }
  return reason;
}

function crossDatasetPipeline(graph, reliableNeighborDatasetIds, sharedCount) {
  const reliableCount = reliableNeighborDatasetIds.length;
  return [
    {
      key: 'scope',
      label: '可见范围',
      value: `${graph.datasets.length} 个数据集`,
      detail: '逐数据集完成权限校验后进入本次图谱。',
      status: 'complete',
      source: 'dataset_semantic_graph_v1.datasets',
      summary: `本次仅展示 ${graph.datasets.length} 个当前可见数据集。`,
      items: graph.datasets.map((dataset) => ({
        id: `scope:${dataset.id}`,
        nodeId: '',
        label: dataset.title,
        meta: dataset.stale ? '上一版快照' : '当前快照',
        detail: '已通过当前请求的逐数据集可见性校验。',
        evidence: '跨数据集图谱可见范围',
        status: dataset.stale ? 'attention' : 'complete',
      })),
    },
    {
      key: 'shared',
      label: '共享识别',
      value: `${sharedCount} 个共享节点`,
      detail: '只把有确定身份依据的资料、字段或概念显示为共享节点。',
      status: sharedCount ? 'complete' : 'empty',
      source: 'dataset_semantic_graph_v1.nodes',
      summary: sharedCount ? `识别到 ${sharedCount} 个可追溯共享节点。` : '尚未识别到可追溯共享节点。',
      items: [],
    },
    {
      key: 'relations',
      label: '关系证据',
      value: `${reliableCount} 个可靠邻居`,
      detail: '确认或观察关系使用实线；相似线索使用虚线。',
      status: reliableCount ? 'complete' : 'empty',
      source: 'dataset_semantic_graph_v1.edges',
      summary: reliableCount
        ? `当前根数据集与 ${reliableCount} 个可见数据集存在确认或观察证据。`
        : '尚未发现有证据的跨数据集共享。',
      items: [],
    },
  ];
}

export function buildCrossDatasetUnderstandingGraph(graph) {
  if (!graph || !Array.isArray(graph.datasets) || !graph.datasets.length) return selectionModel();
  const datasets = [...graph.datasets].sort((left, right) => (
    Number(right.id === graph.root_dataset_id) - Number(left.id === graph.root_dataset_id)
    || left.id.localeCompare(right.id, 'en')
  ));
  const datasetById = new Map(datasets.map((dataset) => [dataset.id, dataset]));
  const datasetClusters = datasets.map((dataset, index) => ({
    id: dataset.id,
    title: dataset.title,
    color: CROSS_DATASET_CLUSTER_COLORS[index % CROSS_DATASET_CLUSTER_COLORS.length],
    stale: dataset.stale,
    root: dataset.id === graph.root_dataset_id,
    nodeCount: 0,
    reliableLinkCount: 0,
  }));
  const clusterById = new Map(datasetClusters.map((cluster) => [cluster.id, cluster]));
  const datasetRootById = new Map();

  const nodes = graph.nodes.map((input) => {
    const datasetRefs = [...input.dataset_refs];
    const sourceDatasets = datasetRefs.map((id) => datasetById.get(id)).filter(Boolean);
    const shared = datasetRefs.length > 1 && CROSS_SHARED_KINDS.has(input.kind);
    const clusterId = datasetRefs.length === 1
      ? datasetRefs[0]
      : `shared:${[...datasetRefs].sort((left, right) => left.localeCompare(right, 'en')).join('|')}`;
    const clusterColors = datasetRefs.map((id) => clusterById.get(id)?.color).filter(Boolean);
    const clusterColor = shared ? CROSS_SHARED_COLOR : clusterColors[0] || CROSS_SHARED_COLOR;
    const item = {
      ...graphNode({
        id: input.id,
        name: input.display_label,
        kind: input.kind,
        detail: shared
          ? `由 ${input.visible_provenance_count} 条当前可见证据共同贡献。`
          : `来自 ${sourceDatasets[0]?.title || '当前可见数据集'}。`,
        evidence: sourceDatasets.map((dataset) => dataset.title).join(' · '),
        status: sourceDatasets.some((dataset) => dataset.stale) ? '上一版可用快照' : '当前可见快照',
        signal: shared ? 'shared' : input.kind,
        symbolSize: input.kind === 'dataset' ? 38 : shared ? 30 : input.kind === 'field' ? 20 : 27,
      }),
      entityType: input.kind,
      datasetRefs,
      sourceDatasets,
      visibleProvenanceCount: input.visible_provenance_count,
      shared,
      sharedBadge: shared ? '共享' : '',
      symbol: shared ? 'diamond' : 'circle',
      clusterId,
      clusterColor,
      clusterColors,
      rootDataset: input.kind === 'dataset' && datasetRefs.includes(graph.root_dataset_id),
      objectId: input.kind === 'field' ? `cluster:${datasetRefs[0] || 'unscoped'}` : '',
      businessScore: (shared ? 100 : 0) + Math.min(80, input.visible_provenance_count),
      technicalOnly: false,
    };
    if (input.kind === 'dataset' && datasetRefs.length === 1 && !datasetRootById.has(datasetRefs[0])) {
      datasetRootById.set(datasetRefs[0], item.id);
    }
    datasetRefs.forEach((id) => {
      const cluster = clusterById.get(id);
      if (cluster) cluster.nodeCount += 1;
    });
    return item;
  });

  datasets.forEach((dataset) => {
    if (datasetRootById.has(dataset.id)) return;
    const cluster = clusterById.get(dataset.id);
    const node = {
      ...graphNode({
        id: `dataset:${dataset.id}`,
        name: dataset.title,
        kind: 'dataset',
        detail: dataset.id === graph.root_dataset_id ? '当前数据集' : '当前可见关联数据集',
        evidence: 'dataset_semantic_graph_v1.datasets',
        status: dataset.stale ? '上一版可用快照' : '当前可见快照',
        signal: 'dataset',
        symbolSize: dataset.id === graph.root_dataset_id ? 42 : 36,
      }),
      entityType: 'dataset',
      datasetRefs: [dataset.id],
      sourceDatasets: [dataset],
      visibleProvenanceCount: 1,
      shared: false,
      sharedBadge: '',
      symbol: 'circle',
      clusterId: dataset.id,
      clusterColor: cluster.color,
      clusterColors: [cluster.color],
      rootDataset: dataset.id === graph.root_dataset_id,
      objectId: '',
      businessScore: 100,
      technicalOnly: false,
    };
    nodes.push(node);
    datasetRootById.set(dataset.id, node.id);
    cluster.nodeCount += 1;
  });

  const links = graph.edges.map((input) => {
    const similarity = input.relation_semantics === 'similarity';
    const reason = similarity ? safeSimilarityReason(input.reason) : input.reason;
    const supportingDatasets = input.supporting_dataset_ids
      .map((id) => datasetById.get(id))
      .filter(Boolean);
    return {
      id: input.id,
      source: input.source_id,
      target: input.target_id,
      relation: similarity ? safeSimilarityRelationLabel(input.label) : input.label,
      relationType: input.relation_type,
      relationSemantics: input.relation_semantics,
      type: similarity ? 'inferred' : input.evidence_class,
      evidenceClass: input.evidence_class,
      confidence: input.confidence,
      evidence: reason,
      reason,
      crossDataset: input.cross_dataset,
      supportingDatasetIds: [...input.supporting_dataset_ids],
      supportingDatasets,
      visibleContributionCount: supportingDatasets.length,
      lineKind: similarity ? 'dashed' : 'solid',
      rootRelation: false,
      structural: input.relation_semantics === 'structure',
    };
  });

  const pairKeys = new Set(links.flatMap((link) => [
    `${link.source}|${link.target}`,
    `${link.target}|${link.source}`,
  ]));
  nodes.filter((node) => node.kind !== 'dataset').forEach((node) => {
    node.datasetRefs.forEach((datasetId) => {
      const rootNodeId = datasetRootById.get(datasetId);
      if (!rootNodeId || pairKeys.has(`${rootNodeId}|${node.id}`)) return;
      const dataset = datasetById.get(datasetId);
      const id = `cluster:${datasetId}:${node.id}`;
      links.push({
        id,
        source: rootNodeId,
        target: node.id,
        relation: '可见归属',
        relationType: 'dataset_scope',
        relationSemantics: 'structure',
        type: 'observed',
        evidenceClass: 'observed',
        confidence: 1,
        evidence: '节点的可见数据集范围。',
        reason: '节点的可见数据集范围。',
        crossDataset: false,
        supportingDatasetIds: [datasetId],
        supportingDatasets: dataset ? [dataset] : [],
        visibleContributionCount: 1,
        lineKind: 'solid',
        rootRelation: true,
        structural: true,
      });
      pairKeys.add(`${rootNodeId}|${node.id}`);
      pairKeys.add(`${node.id}|${rootNodeId}`);
    });
  });

  withSemanticNeighborhoods(nodes, links);
  const reliableLinks = links.filter((link) => (
    link.crossDataset
    && link.relationSemantics !== 'similarity'
    && ['confirmed', 'observed'].includes(link.evidenceClass)
  ));
  const sharedNodes = nodes.filter((node) => node.shared);
  const reliableNeighborDatasetIds = [...new Set([
    ...reliableLinks.flatMap((link) => link.supportingDatasetIds),
    ...sharedNodes.flatMap((node) => node.datasetRefs),
  ].filter((id) => id !== graph.root_dataset_id))]
    .sort((left, right) => left.localeCompare(right, 'en'));
  reliableLinks.forEach((link) => link.supportingDatasetIds.forEach((id) => {
    if (id === graph.root_dataset_id) return;
    const cluster = clusterById.get(id);
    if (cluster) cluster.reliableLinkCount += 1;
  }));
  sharedNodes.forEach((node) => node.datasetRefs.forEach((id) => {
    if (id === graph.root_dataset_id) return;
    const cluster = clusterById.get(id);
    if (cluster) cluster.reliableLinkCount += 1;
  }));
  const crossLinks = links.filter((link) => link.crossDataset);
  const confirmedLinks = crossLinks.filter((link) => link.evidenceClass === 'confirmed');
  const observedLinks = crossLinks.filter((link) => link.evidenceClass === 'observed');
  const inferredLinks = crossLinks.filter((link) => link.evidenceClass === 'inferred');
  const rootDataset = datasetById.get(graph.root_dataset_id) || datasets[0];
  const emptyCrossMessage = reliableNeighborDatasetIds.length
    ? ''
    : '尚未发现有证据的跨数据集共享；系统不会使用纯相似度凑推荐。';

  return {
    hasDataset: true,
    mode: 'cross',
    viewLabel: '跨数据集语义图',
    overviewTitle: '跨数据集共享与引用',
    snapshotStatus: graph.cross_links_status,
    crossLinksStatus: graph.cross_links_status,
    statusMessage: graph.stale
      ? '当前展示上一版可用跨数据集图谱。'
      : '当前只展示本次可见范围内的共享、引用与相似线索。',
    stale: graph.stale,
    generatedAt: '',
    limitations: [
      '相似关系仅作为推断线索，不折叠节点，也不升级为确定身份或确定共享关系。',
      ...(graph.truncated.nodes || graph.truncated.edges ? ['后端响应已按安全上限截断。'] : []),
    ],
    datasetId: graph.root_dataset_id,
    rootDatasetId: graph.root_dataset_id,
    title: rootDataset?.title || '当前数据集',
    categories: DATASET_GRAPH_CATEGORIES,
    datasetClusters,
    hasReliableCrossLinks: reliableNeighborDatasetIds.length > 0,
    reliableNeighborDatasetIds,
    emptyCrossMessage,
    metrics: {
      documentCount: nodes.filter((node) => node.kind === 'document').length,
      estimatedWordCount: 0,
      parsedDocumentCount: 0,
      readyDocumentCount: datasets.filter((dataset) => !dataset.stale).length,
      attentionDocumentCount: datasets.filter((dataset) => dataset.stale).length,
      knowledgeCount: nodes.filter((node) => node.kind === 'concept').length,
      observedRelationCount: observedLinks.length,
      inferredRelationCount: inferredLinks.length,
      confirmedRelationCount: confirmedLinks.length,
      crossNodeRelationCount: crossLinks.length,
      sourceCount: datasets.length,
      objectCount: nodes.filter((node) => node.kind === 'object').length,
      fieldCount: nodes.filter((node) => node.kind === 'field').length,
      unresolvedFieldCount: 0,
      confirmedFactCount: confirmedLinks.length,
      sharedNodeCount: sharedNodes.length,
    },
    pipeline: crossDatasetPipeline(graph, reliableNeighborDatasetIds, sharedNodes.length),
    understanding: {
      summary: reliableNeighborDatasetIds.length
        ? `当前数据集与 ${reliableNeighborDatasetIds.length} 个可见邻居存在确认或观察证据，识别到 ${sharedNodes.length} 个共享节点。`
        : emptyCrossMessage,
      keyConcepts: cleanList(sharedNodes.map((node) => node.name), 10),
      keyFields: cleanList(nodes.filter((node) => node.kind === 'field').map((node) => node.name), 10),
      technicalIdentifiers: [],
      structurePath: datasets.map((dataset) => dataset.title),
      strategies: [],
      retrievalCoverage: {
        ready: datasets.filter((dataset) => !dataset.stale).length,
        total: datasets.length,
      },
    },
    nodes,
    links,
    emptyKnowledgeMessage: emptyCrossMessage,
    fallbackQuality: {
      hiddenCount: 0,
      hiddenItems: [],
      hiddenByClass: {},
      deduplicatedDocumentTitles: 0,
    },
    truncated: graph.truncated,
  };
}

export function layoutCrossDatasetUnderstandingGraph(inputNodes, inputClusters = []) {
  const nodes = (Array.isArray(inputNodes) ? inputNodes : []).filter((node) => node?.id);
  const datasetKey = (node) => [...new Set(node.datasetRefs || [])].sort().join('|');
  const orderedNodes = [...nodes].sort((left, right) => {
    const rank = (node) => node.kind === 'dataset' ? 0 : node.shared ? 1 : 2;
    return rank(left) - rank(right)
      || datasetKey(left).localeCompare(datasetKey(right))
      || String(left.id).localeCompare(String(right.id));
  });
  const represented = new Set(nodes.flatMap((node) => node.datasetRefs || []));
  const clusters = (Array.isArray(inputClusters) ? inputClusters : [])
    .filter((cluster) => represented.has(cluster.id))
    .sort((left, right) => String(left.id).localeCompare(String(right.id)));
  const centers = new Map();
  const clusterRadius = clusters.length <= 1 ? 0 : Math.min(540, 270 + clusters.length * 32);
  clusters.forEach((cluster, index) => {
    const angle = -Math.PI / 2 + Math.PI * 2 * index / Math.max(1, clusters.length);
    centers.set(cluster.id, {
      x: Number((Math.cos(angle) * clusterRadius).toFixed(3)),
      y: Number((Math.sin(angle) * clusterRadius).toFixed(3)),
    });
  });
  const memberCounts = new Map();
  orderedNodes.forEach((node) => {
    if (node.kind === 'dataset' || node.shared) return;
    const datasetId = [...new Set(node.datasetRefs || [])].sort()[0];
    memberCounts.set(datasetId, (memberCounts.get(datasetId) || 0) + 1);
  });
  const memberIndexes = new Map();
  const memberPositions = new Map();
  orderedNodes.filter((node) => node.kind !== 'dataset' && !node.shared).forEach((node) => {
    const datasetId = [...new Set(node.datasetRefs || [])].sort()[0];
    const center = centers.get(datasetId) || { x: 0, y: 0 };
    const index = memberIndexes.get(datasetId) || 0;
    memberIndexes.set(datasetId, index + 1);
    const ring = Math.floor(index / 14);
    const angle = Math.PI * 2 * (index % 14)
      / Math.min(14, Math.max(1, memberCounts.get(datasetId) || 1));
    const radius = 105 + ring * 68;
    memberPositions.set(node.id, {
      x: Number((center.x + Math.cos(angle) * radius).toFixed(3)),
      y: Number((center.y + Math.sin(angle) * radius).toFixed(3)),
      layoutTier: Math.min(3, ring + 1),
    });
  });
  const sharedPositions = new Map();
  const occupiedSharedPositions = [];
  const sharedSlotOrder = [0, 4, 2, 6, 1, 3, 5, 7];
  const reservedPositions = orderedNodes.filter((node) => node.kind === 'dataset').map((node) => ({
    ...(centers.get([...new Set(node.datasetRefs || [])].sort()[0]) || { x: 0, y: 0 }),
    radius: (node.rootDataset ? 42 : 36) / 2,
  }));
  orderedNodes.filter((node) => node.kind !== 'dataset' && !node.shared).forEach((node) => {
    const position = memberPositions.get(node.id) || { x: 0, y: 0 };
    reservedPositions.push({
      x: position.x,
      y: position.y,
      radius: Math.max(12, (Number(node.symbolSize) || 24) / 2),
    });
  });
  const sharedNodes = orderedNodes.filter((node) => node.shared);
  sharedNodes.forEach((node) => {
    const scopedCenters = (node.datasetRefs || []).map((id) => centers.get(id)).filter(Boolean);
    const base = scopedCenters.length ? {
      x: scopedCenters.reduce((sum, center) => sum + center.x, 0) / scopedCenters.length,
      y: scopedCenters.reduce((sum, center) => sum + center.y, 0) / scopedCenters.length,
    } : { x: 0, y: 0 };
    let chosen = null;
    const maximumRings = Math.max(8, sharedNodes.length + reservedPositions.length + 4);
    for (let ring = 0; ring < maximumRings && !chosen; ring += 1) {
      const radius = 72 + ring * 48;
      for (const slot of sharedSlotOrder) {
        const angle = -Math.PI / 2 + Math.PI * 2 * slot / sharedSlotOrder.length;
        const candidate = {
          x: Number((base.x + Math.cos(angle) * radius).toFixed(3)),
          y: Number((base.y + Math.sin(angle) * radius).toFixed(3)),
        };
        const clearsSharedNodes = occupiedSharedPositions.every((position) => (
          Math.hypot(position.x - candidate.x, position.y - candidate.y) >= 48
        ));
        const clearsOtherNodes = reservedPositions.every((position) => (
          Math.hypot(position.x - candidate.x, position.y - candidate.y) >= position.radius + 20
        ));
        if (clearsSharedNodes && clearsOtherNodes) {
          chosen = candidate;
          break;
        }
      }
    }
    if (!chosen) throw new Error(`unable to place shared graph node: ${String(node.id)}`);
    occupiedSharedPositions.push(chosen);
    sharedPositions.set(node.id, chosen);
  });
  const positioned = new Map();
  orderedNodes.forEach((node) => {
    if (node.kind === 'dataset') {
      const center = centers.get(node.datasetRefs?.[0]) || { x: 0, y: 0 };
      positioned.set(node.id, {
        ...node,
        ...center,
        fixed: true,
        layoutTier: 0,
        symbolSize: node.rootDataset ? 42 : 36,
      });
      return;
    }
    if (node.shared) {
      positioned.set(node.id, {
        ...node,
        ...(sharedPositions.get(node.id) || { x: 0, y: 0 }),
        fixed: false,
        layoutTier: 1,
        symbol: 'diamond',
        symbolSize: Math.max(24, Number(node.symbolSize) || 0),
      });
      return;
    }
    const position = memberPositions.get(node.id) || { x: 0, y: 0, layoutTier: 1 };
    positioned.set(node.id, {
      ...node,
      x: position.x,
      y: position.y,
      fixed: false,
      layoutTier: position.layoutTier,
    });
  });
  return nodes.map((node) => positioned.get(node.id) || node);
}

function semanticPipelineItems(stage, understanding, nodeById) {
  if (stage.id === 'source') {
    return understanding.source_groups.flatMap((group) => group.object_ids.map((objectId) => ({
      id: `${stage.id}:${group.id}:${objectId}`,
      nodeId: objectId,
      label: nodeById.get(objectId)?.name || group.label,
      meta: group.label || group.kind,
      detail: `覆盖 ${group.coverage_count} 条来源信号`,
      evidence: '语义快照 source_groups',
      status: 'complete',
    })));
  }
  if (stage.id === 'structure') {
    return understanding.objects.map((object) => ({
      id: `${stage.id}:${object.id}`,
      nodeId: object.id,
      label: nodeById.get(object.id)?.name || semanticObjectDisplayLabel(object),
      meta: semanticStatusLabel(object.status),
      detail: object.description || `覆盖 ${object.coverage_count} 条记录`,
      evidence: semanticEvidenceText(object.evidence_refs),
      status: object.status === 'unresolved' ? 'attention' : 'complete',
    }));
  }
  if (stage.id === 'labels') {
    return understanding.fields.map((field) => ({
      id: `${stage.id}:${field.id}`,
      nodeId: field.id,
      label: nodeById.get(field.id)?.name || '待解释字段',
      meta: `${semanticStatusLabel(field.status)} · ${field.semantic_role}`,
      detail: `${field.value_type} · 非空 ${field.non_empty_count} · 去重 ${field.distinct_count}`,
      evidence: semanticEvidenceText(field.evidence_refs),
      status: field.status === 'unresolved' ? 'attention' : 'complete',
    }));
  }
  if (stage.id === 'relations') {
    return understanding.relations.map((relation) => ({
      id: `${stage.id}:${relation.id}`,
      nodeId: relation.source_id,
      label: relation.label,
      meta: semanticStatusLabel(relation.evidence_class),
      detail: relation.reason,
      evidence: semanticEvidenceText(relation.evidence_refs),
      status: relation.evidence_class === 'inferred' ? 'attention' : 'complete',
    }));
  }
  if (stage.id === 'facts') {
    return understanding.relations
      .filter((relation) => relation.evidence_class === 'confirmed')
      .map((relation) => ({
        id: `${stage.id}:${relation.id}`,
        nodeId: relation.source_id,
        label: relation.label,
        meta: '已确认关系',
        detail: relation.reason,
        evidence: semanticEvidenceText(relation.evidence_refs),
        status: 'complete',
      }));
  }
  return [];
}

function buildSemanticDatasetUnderstandingGraph(dataset, understanding) {
  const datasetId = String(dataset?.id || understanding.dataset.id);
  const rootId = `dataset:${datasetId}`;
  const sourceGroupByObject = new Map();
  understanding.source_groups.forEach((group) => group.object_ids.forEach((objectId) => {
    sourceGroupByObject.set(objectId, { id: group.id, label: group.label, kind: group.kind });
  }));
  const nodes = [graphNode({
    id: rootId,
    name: cleanText(dataset?.title) || understanding.dataset.title || '当前数据集',
    kind: 'dataset',
    detail: understanding.summary.headline,
    evidence: 'dataset_semantic_understanding_v1',
    status: understanding.stale ? '上一版可用快照' : '最新可用快照',
    symbolSize: 68,
  })];

  understanding.objects.forEach((object) => {
    const group = sourceGroupByObject.get(object.id);
    const displayLabel = semanticObjectDisplayLabel(object);
    nodes.push({
      ...graphNode({
        id: object.id,
        name: displayLabel,
        kind: object.status === 'unresolved' ? 'unresolved' : 'object',
        detail: object.description || `系统从 ${object.kind} 中识别出的业务对象。`,
        evidence: semanticEvidenceText(object.evidence_refs),
        status: semanticStatusLabel(object.status),
        signal: object.status,
        symbolSize: semanticNodeSize(object.coverage_count, 32, 54),
      }),
      entityType: 'object',
      rawLabel: object.label,
      sourceKind: object.kind,
      technicalName: object.technical_name,
      labelSource: object.label_source,
      confidence: object.confidence,
      coverageCount: object.coverage_count,
      evidenceRefs: object.evidence_refs,
      groupId: group?.id || '',
      groupLabel: group?.label || '',
      technicalOnly: !conciseChineseLabel(object.label),
    });
  });

  const objectDisplayLabels = new Map(nodes
    .filter((node) => node.entityType === 'object')
    .map((node) => [node.id, node.name]));

  understanding.fields.forEach((field) => {
    const unresolved = field.status === 'unresolved';
    const displayLabel = semanticFieldDisplayLabel(field, objectDisplayLabels.get(field.object_id));
    nodes.push({
      ...graphNode({
        id: field.id,
        name: displayLabel,
        kind: unresolved ? 'unresolved' : 'field',
        detail: unresolved
          ? '该字段已识别结构和值域，但业务含义尚未确认。'
          : `${field.semantic_role} · ${field.value_type}`,
        evidence: semanticEvidenceText(field.evidence_refs),
        status: semanticStatusLabel(field.status),
        signal: field.status,
        symbolSize: unresolved ? 16 : semanticNodeSize(field.non_empty_count, 19, 31),
      }),
      entityType: 'field',
      rawLabel: field.label,
      objectId: field.object_id,
      technicalName: field.technical_name,
      semanticRole: field.semantic_role,
      valueType: field.value_type,
      nonEmptyCount: field.non_empty_count,
      distinctCount: field.distinct_count,
      examples: field.examples,
      labelSource: field.label_source,
      confidence: field.confidence,
      evidenceRefs: field.evidence_refs,
      technicalOnly: unresolved || !trustedBusinessFieldLabel(field),
      businessScore: fieldBusinessScore(field),
    });
  });

  const links = [];
  understanding.objects.forEach((object, index) => links.push({
    id: `structural:dataset:${object.id}`,
    source: rootId,
    target: object.id,
    relation: '包含业务对象',
    type: 'observed',
    confidence: 1,
    evidence: '语义快照 dataset / objects 归属',
    rootRelation: true,
    structural: true,
    order: index,
  }));

  understanding.relations.forEach((relation) => links.push({
    id: relation.id,
    source: relation.source_id,
    target: relation.target_id,
    relation: relation.label,
    relationType: relation.relation_type,
    type: relation.evidence_class,
    confidence: relation.confidence,
    evidence: relation.reason || semanticEvidenceText(relation.evidence_refs),
    evidenceRefs: relation.evidence_refs,
    rootRelation: false,
    structural: relation.relation_type === 'contains' || relation.relation_type === 'membership',
  }));

  const relationPairs = new Set(understanding.relations.map((relation) => `${relation.source_id}|${relation.target_id}`));
  understanding.fields.forEach((field) => {
    if (relationPairs.has(`${field.object_id}|${field.id}`)) return;
    links.push({
      id: `structural:field:${field.id}`,
      source: field.object_id,
      target: field.id,
      relation: '包含字段',
      type: 'observed',
      confidence: 1,
      evidence: '语义快照 fields.object_id 归属',
      rootRelation: false,
      structural: true,
    });
  });
  withSemanticNeighborhoods(nodes, links);
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const pipeline = understanding.pipeline.map((stage) => {
    const items = semanticPipelineItems(stage, understanding, nodeById);
    const attention = items.some((item) => item.status === 'attention');
    return {
      key: stage.id,
      label: stage.label,
      value: `${stage.evidence_count} 条信号`,
      detail: stage.detail,
      status: attention ? 'attention' : stage.status === 'completed' || stage.status === 'complete' ? 'complete' : 'empty',
      source: 'dataset_semantic_understanding_v1.pipeline',
      summary: `${stage.label}：${stage.detail}`,
      items,
    };
  });
  const confirmedRelations = understanding.relations.filter((relation) => relation.evidence_class === 'confirmed');
  const observedRelations = understanding.relations.filter((relation) => relation.evidence_class === 'observed');
  const inferredRelations = understanding.relations.filter((relation) => relation.evidence_class === 'inferred');
  const resolvedFields = understanding.fields.filter((field) => field.status !== 'unresolved');
  const unresolvedFields = understanding.fields.filter((field) => field.status === 'unresolved');
  const documentCount = understanding.coverage.document_count;
  const retrievalCount = understanding.coverage.retrieval_evidence_count;

  return {
    hasDataset: true,
    mode: 'semantic',
    viewLabel: '真实语义快照',
    overviewTitle: '系统已经理解到什么',
    snapshotStatus: understanding.status,
    statusMessage: understanding.stale ? '当前展示上一版可用理解快照，后台最新重建尚未成功完成。' : '当前展示最新可用语义理解快照。',
    stale: understanding.stale,
    generatedAt: understanding.generated_at,
    limitations: understanding.summary.limitations,
    datasetId,
    title: cleanText(dataset?.title) || understanding.dataset.title || '当前数据集',
    categories: DATASET_GRAPH_CATEGORIES,
    metrics: {
      documentCount,
      estimatedWordCount: numericValue(dataset?.estimated_word_count ?? dataset?.estimatedWordCount),
      parsedDocumentCount: documentCount,
      readyDocumentCount: retrievalCount,
      attentionDocumentCount: unresolvedFields.length,
      knowledgeCount: understanding.objects.length + resolvedFields.length,
      confirmedRelationCount: confirmedRelations.length,
      observedRelationCount: observedRelations.length,
      inferredRelationCount: inferredRelations.length,
      crossNodeRelationCount: understanding.relations.length,
      sourceCount: understanding.coverage.source_count,
      objectCount: understanding.objects.length,
      fieldCount: understanding.fields.length,
      unresolvedFieldCount: unresolvedFields.length,
      confirmedFactCount: understanding.coverage.confirmed_fact_count,
      recordCount: understanding.coverage.record_count,
      assetCount: understanding.coverage.asset_count,
    },
    pipeline,
    understanding: {
      summary: understanding.summary.headline,
      keyConcepts: cleanList([
        ...understanding.objects.filter((object) => object.status !== 'unresolved')
          .map((object) => nodeById.get(object.id)?.name),
        ...resolvedFields.filter((field) => trustedBusinessFieldLabel(field))
          .map((field) => nodeById.get(field.id)?.name),
      ], 16),
      keyFields: cleanList(
        resolvedFields.filter((field) => trustedBusinessFieldLabel(field))
          .map((field) => nodeById.get(field.id)?.name),
        16,
      ),
      technicalIdentifiers: cleanList([
        ...understanding.objects.filter((object) => object.status === 'unresolved').map((object) => object.technical_name),
        ...unresolvedFields.map((field) => field.technical_name),
      ], 40),
      structurePath: understanding.objects.filter((object) => object.status !== 'unresolved')
        .map((object) => nodeById.get(object.id)?.name),
      strategies: understanding.pipeline.map((stage) => stage.label),
      retrievalCoverage: { ready: retrievalCount, total: documentCount },
    },
    nodes,
    links,
    emptyKnowledgeMessage: resolvedFields.length || understanding.objects.length
      ? ''
      : '当前语义快照尚未返回可解释业务对象或字段。',
  };
}

export function buildDatasetUnderstandingGraph(dataset, documents = [], semanticUnderstanding = null) {
  if (!dataset?.id) return selectionModel();

  if (
    semanticUnderstanding?.status === 'ready'
    && Array.isArray(semanticUnderstanding.objects)
    && semanticUnderstanding.objects.length
  ) {
    return buildSemanticDatasetUnderstandingGraph(dataset, semanticUnderstanding);
  }

  const datasetId = String(dataset.id);
  const scopedDocuments = (Array.isArray(documents) ? documents : [])
    .filter((document) => datasetDocumentIds(document).includes(datasetId));
  const rawKnowledgeTerms = cleanList(
    dataset.noun_term_hints || dataset.nounTermHints || dataset.noun_terms || dataset.nounTerms,
  );
  const rawSectionTitles = cleanList(dataset.section_title_hints || dataset.sectionTitleHints);
  const rawUnderstandingStrategies = cleanList(
    dataset.document_understanding_strategies || dataset.documentUnderstandingStrategies,
  );
  const materialHints = cleanList(dataset.material_hints || dataset.materialHints);
  const explicitContentTypes = summaryItems(dataset.content_type_summary || dataset.contentTypeSummary);
  const documentContentTypes = cleanList(
    scopedDocuments.map((document) => documentTypeLabel(document.content_type || document.contentType)),
  );
  const rawMaterialTypes = cleanList(
    [...documentContentTypes, ...explicitContentTypes, ...materialHints],
  );
  const knowledgeProjection = projectedFallbackItems(rawKnowledgeTerms, 'knowledge', LIMITS.knowledge);
  const sectionProjection = projectedFallbackItems(rawSectionTitles, 'section', LIMITS.section);
  const strategyProjection = projectedFallbackItems(rawUnderstandingStrategies, 'strategy', LIMITS.strategy);
  const materialProjection = projectedFallbackItems(rawMaterialTypes, 'material', LIMITS.material);
  const knowledgeTerms = knowledgeProjection.allowed;
  const sectionTitles = sectionProjection.allowed;
  const understandingStrategies = strategyProjection.allowed;
  const materialTypes = materialProjection.allowed;

  const parseSummary = cleanText(dataset.parse_status_summary || dataset.parseStatusSummary);
  const actualDocumentCount = scopedDocuments.length;
  const documentCount = actualDocumentCount || numericValue(
    dataset.document_count ?? dataset.documents_count ?? dataset.documentCount ?? dataset.documentsCount,
  );
  const parsedCount = actualDocumentCount
    ? scopedDocuments.filter(parsedDocument).length
    : parseSummaryCount(parseSummary, ['completed', 'complete', 'parsed', 'indexed']);
  const readyCount = actualDocumentCount
    ? scopedDocuments.filter(readyDocument).length
    : parseSummaryCount(parseSummary, ['indexed', 'ready']);
  const attentionCount = actualDocumentCount
    ? scopedDocuments.filter(attentionDocument).length
    : parseSummaryCount(parseSummary, ['attention_required', 'attention', 'failed']);
  const estimatedWordCount = numericValue(
    dataset.estimated_word_count ?? dataset.estimatedWordCount ?? dataset.word_count ?? dataset.wordCount,
  );

  const rootId = `dataset:${datasetId}`;
  const nodes = [graphNode({
    id: rootId,
    name: cleanText(dataset.title) || cleanText(dataset.key) || '当前数据集',
    kind: 'dataset',
    detail: `当前图谱中心，共连接 ${documentCount} 份资料和 ${knowledgeTerms.length} 个已返回知识词。`,
    evidence: '数据集摘要接口',
    status: cleanText(dataset.parse_status_summary || dataset.parseStatusSummary) || cleanText(dataset.lifecycle),
    symbolSize: 66,
  })];
  const links = [];
  const linkPairs = new Set();

  const addLink = ({ source, target, relation, type = 'observed', confidence = 1, evidence }) => {
    if (!source || !target || source === target) return null;
    const pairKey = [source, target].sort().join('|');
    if (linkPairs.has(pairKey)) return null;
    linkPairs.add(pairKey);
    const link = {
      id: `link:${links.length + 1}`,
      source,
      target,
      relation,
      type,
      confidence: Math.max(0, Math.min(1, Number(confidence) || 0)),
      evidence: cleanText(evidence) || '当前数据集已有字段',
      rootRelation: source === rootId || target === rootId,
    };
    links.push(link);
    return link;
  };

  const addConnectedNodes = (items, kind, builder) => {
    return items.map((item, index) => {
      const node = builder(item, index);
      nodes.push(node);
      addLink({
        source: rootId,
        target: node.id,
        relation: kind === 'document' ? '包含' : kind === 'strategy' ? '采用' : '识别',
        type: 'observed',
        confidence: 1,
        evidence: node.evidence,
      });
      return node;
    });
  };

  const documentCandidates = [];
  const documentStageRecords = [];
  const knownCanonicalTitles = new Set();
  const seenDocumentKeys = new Set();
  let deduplicatedDocumentTitles = 0;
  scopedDocuments.forEach((document, index) => {
    const rawLabel = cleanText(document.title) || cleanText(document.object_key) || `未命名文档 ${index + 1}`;
    const projection = projectFallbackLabel(rawLabel, { kind: 'document' });
    const materialName = documentTypeLabel(document.content_type || document.contentType);
    const canonicalTitle = canonicalDocumentTitle(rawLabel) || `untitled-${index}`;
    const candidate = {
      id: document.id ? `document:${document.id}` : nodeId('document', canonicalTitle, index),
      rawLabel,
      projection,
      canonicalTitle,
      name: projection.display_label,
      detail: `${materialName || '未知格式'} · ${documentParseStatus(document) || documentLifecycle(document) || '状态未知'} · ${projection.detail_label}`,
      status: documentQualityStatus(document) || documentParseStatus(document) || documentLifecycle(document),
      evidence: '当前数据集文档列表',
      materialName,
    };
    documentStageRecords.push({ document, candidate });
    knownCanonicalTitles.add(canonicalTitle);
    const candidateKey = `${canonicalTitle}|${normalizedLabel(materialName) || 'unknown'}`;
    if (seenDocumentKeys.has(candidateKey)) {
      deduplicatedDocumentTitles += 1;
      return;
    }
    seenDocumentKeys.add(candidateKey);
    documentCandidates.push(candidate);
  });
  cleanList(dataset.document_title_hints || dataset.documentTitleHints).forEach((rawLabel, index) => {
    const canonicalTitle = canonicalDocumentTitle(rawLabel) || `hint-${index}`;
    if (knownCanonicalTitles.has(canonicalTitle) || seenDocumentKeys.has(`${canonicalTitle}|hint`)) {
      deduplicatedDocumentTitles += 1;
      return;
    }
    seenDocumentKeys.add(`${canonicalTitle}|hint`);
    const projection = projectFallbackLabel(rawLabel, { kind: 'document' });
    documentCandidates.push({
      id: nodeId('document', canonicalTitle, index),
      rawLabel,
      projection,
      canonicalTitle,
      name: projection.display_label,
      detail: `数据集摘要返回的文档标题线索 · ${projection.detail_label}`,
      status: '标题线索',
      evidence: '数据集 document_title_hints',
      materialName: '',
    });
  });
  const hiddenDocumentItems = documentCandidates
    .filter((item) => !item.projection.main_canvas_allowed)
    .map((item, index) => ({
      id: `hidden:document:${index}`,
      kind: 'document',
      label: item.projection.display_label,
      qualityClass: item.projection.quality_class,
      reason: item.projection.reason,
      detail: item.projection.detail_label,
    }));
  const documentNodes = documentCandidates
    .filter((item) => item.projection.main_canvas_allowed)
    .slice(0, LIMITS.document);
  const createdDocumentNodes = addConnectedNodes(documentNodes, 'document', (item) => ({
    ...graphNode({ ...item, kind: 'document', symbolSize: 18 }),
    technicalName: item.rawLabel,
    qualityClass: item.projection.quality_class,
  }));
  const createdKnowledgeNodes = addConnectedNodes(knowledgeTerms, 'knowledge', (term, index) => graphNode({
    id: nodeId('knowledge', term.normalized_key || term.display_label, index),
    name: term.display_label,
    kind: 'knowledge',
    detail: `数据集摘要返回的可信中文知识线索 · ${term.detail_label}`,
    evidence: '数据集 noun_term_hints',
    signal: 'concept',
    symbolSize: 28,
  }));
  const createdSectionNodes = addConnectedNodes(sectionTitles, 'section', (title, index) => graphNode({
    id: nodeId('section', title.normalized_key || title.display_label, index),
    name: title.display_label,
    kind: 'section',
    detail: `解析摘要返回的可信中文结构线索 · ${title.detail_label}`,
    evidence: '数据集 section_title_hints',
    symbolSize: 21,
  }));
  const createdMaterialNodes = addConnectedNodes(materialTypes, 'material', (material, index) => graphNode({
    id: nodeId('material', material.normalized_key || material.display_label, index),
    name: material.display_label,
    kind: 'material',
    detail: '当前数据集已有的资料类型或材料线索。',
    evidence: explicitContentTypes.length ? '数据集 content_type_summary / material_hints' : '文档 content_type / 数据集 material_hints',
    symbolSize: 18,
  }));
  const createdStrategyNodes = addConnectedNodes(understandingStrategies, 'strategy', (strategy, index) => graphNode({
    id: nodeId('strategy', strategy.normalized_key || strategy.display_label, index),
    name: strategy.display_label,
    kind: 'strategy',
    detail: '当前数据集摘要记录的文档理解策略。',
    evidence: '数据集 document_understanding_strategies',
    symbolSize: 23,
  }));

  const materialNodeByName = new Map(createdMaterialNodes.map((node) => [normalizedLabel(node.name), node]));
  documentNodes.forEach((documentNode, index) => {
    const materialNode = materialNodeByName.get(normalizedLabel(documentNode.materialName));
    if (!materialNode || !createdDocumentNodes[index]) return;
    addLink({
      source: createdDocumentNodes[index].id,
      target: materialNode.id,
      relation: '资料格式',
      type: 'observed',
      confidence: 1,
      evidence: '文档 content_type',
    });
  });

  const addSparseGroupLinks = (groupNodes, relation, evidence, confidence) => {
    if (groupNodes.length < 2) return;
    groupNodes.slice(0, -1).forEach((node, index) => {
      addLink({
        source: node.id,
        target: groupNodes[index + 1].id,
        relation,
        type: 'inferred',
        confidence,
        evidence,
      });
    });
    if (groupNodes.length > 2) {
      addLink({
        source: groupNodes[groupNodes.length - 1].id,
        target: groupNodes[0].id,
        relation,
        type: 'inferred',
        confidence,
        evidence,
      });
    }
  };

  const addAffinityLinks = (sourceNodes, targetNodes, relation) => {
    sourceNodes.forEach((sourceNode) => {
      targetNodes
        .map((targetNode) => ({ targetNode, ...labelAffinity(sourceNode.name, targetNode.name) }))
        .filter((candidate) => candidate.score >= 0.18)
        .sort((left, right) => right.score - left.score || left.targetNode.id.localeCompare(right.targetNode.id))
        .slice(0, 2)
        .forEach((candidate) => {
          addLink({
            source: sourceNode.id,
            target: candidate.targetNode.id,
            relation,
            type: 'inferred',
            confidence: Math.min(0.88, 0.52 + candidate.score * 0.5),
            evidence: `名称文本片段“${candidate.sharedTokens.slice(0, 2).join('、')}”在两侧同时出现`,
          });
        });
    });
  };

  addAffinityLinks(createdDocumentNodes, createdKnowledgeNodes, '词义线索');
  addAffinityLinks(createdDocumentNodes, createdSectionNodes, '结构线索');
  addAffinityLinks(createdKnowledgeNodes, createdSectionNodes, '概念呼应');
  addSparseGroupLinks(
    createdKnowledgeNodes,
    '同组线索',
    '同一 dataset noun_term_hints 列表中共同返回；具体语义关系待后端验证',
    0.42,
  );
  addSparseGroupLinks(
    createdSectionNodes,
    '结构邻接',
    '同一 dataset section_title_hints 列表的相邻线索；具体章节顺序待后端验证',
    0.46,
  );
  createdStrategyNodes.forEach((strategyNode, index) => {
    const sectionNode = createdSectionNodes[index % createdSectionNodes.length];
    if (!sectionNode) return;
    addLink({
      source: strategyNode.id,
      target: sectionNode.id,
      relation: '参与识别',
      type: 'inferred',
      confidence: 0.38,
      evidence: '数据集同时返回理解策略与章节线索；具体对应关系待后端验证',
    });
  });

  const hiddenItems = [
    ...hiddenDocumentItems,
    ...knowledgeProjection.hidden,
    ...sectionProjection.hidden,
    ...materialProjection.hidden,
    ...strategyProjection.hidden,
  ];
  const hiddenByClass = hiddenItems.reduce((counts, item) => {
    counts[item.qualityClass] = (counts[item.qualityClass] || 0) + 1;
    return counts;
  }, {});
  const fallbackQuality = {
    hiddenCount: hiddenItems.length,
    hiddenItems,
    hiddenByClass,
    deduplicatedDocumentTitles,
  };
  const metrics = {
    documentCount,
    estimatedWordCount,
    parsedDocumentCount: parsedCount,
    readyDocumentCount: readyCount,
    attentionDocumentCount: attentionCount,
    knowledgeCount: knowledgeTerms.length + sectionTitles.length,
    observedRelationCount: links.filter((link) => link.type === 'observed').length,
    inferredRelationCount: links.filter((link) => link.type === 'inferred').length,
    crossNodeRelationCount: links.filter((link) => !link.rootRelation).length,
  };
  const visibleNodeIds = new Set(nodes.map((node) => node.id));
  const documentItems = documentStageRecords.map(({ document, candidate }, index) => ({
    id: document.id ? `document:${document.id}` : `document-stage:${index}`,
    nodeId: visibleNodeIds.has(candidate.id) ? candidate.id : '',
    label: candidate.projection.display_label,
    meta: candidate.materialName || '格式未返回',
    detail: `${readableStatus(document.lifecycle)} · ${candidate.projection.detail_label}`,
    evidence: '当前数据集文档列表',
    status: 'complete',
  }));
  const parseItems = documentStageRecords.map(({ document, candidate }, index) => ({
    id: document.id ? `parse:${document.id}` : `parse-stage:${index}`,
    nodeId: visibleNodeIds.has(candidate.id) ? candidate.id : '',
    label: candidate.projection.display_label,
    meta: parsedDocument(document) ? '已解析' : '待解析',
    detail: attentionDocument(document)
      ? `质量状态：${readableStatus(documentQualityStatus(document), '需要关注')} · ${candidate.projection.detail_label}`
      : `解析状态：${readableStatus(documentParseStatus(document) || documentLifecycle(document))} · ${candidate.projection.detail_label}`,
    evidence: '文档 parse_status / parse_quality_status / lifecycle',
    status: attentionDocument(document) ? 'attention' : parsedDocument(document) ? 'complete' : 'empty',
  }));
  const structureItems = sectionTitles.map((title, index) => ({
      id: `section-stage:${index}`,
      nodeId: nodeId('section', title.normalized_key || title.display_label, index),
      label: title.display_label,
      meta: '结构线索',
      detail: `解析摘要返回的可信中文章节或结构标题 · ${title.detail_label}`,
      evidence: '数据集 section_title_hints',
      status: 'complete',
    }));
  const knowledgeItems = knowledgeTerms.map((term, index) => ({
    id: `knowledge-stage:${index}`,
    nodeId: nodeId('knowledge', term.normalized_key || term.display_label, index),
    label: term.display_label,
    meta: '核心概念',
    detail: `接口返回且通过质量门禁的中文知识词 · ${term.detail_label}`,
    evidence: '数据集 noun_term_hints',
    status: 'complete',
  }));
  const readyItems = documentStageRecords.filter(({ document }) => readyDocument(document)).map(({ document, candidate }, index) => ({
    id: document.id ? `ready:${document.id}` : `ready-stage:${index}`,
    nodeId: visibleNodeIds.has(candidate.id) ? candidate.id : '',
    label: candidate.projection.display_label,
    meta: '可检索',
    detail: `生命周期：${readableStatus(document.lifecycle)} · 解析：${readableStatus(documentParseStatus(document))} · ${candidate.projection.detail_label}`,
    evidence: '文档 lifecycle / parse_status',
    status: 'complete',
  }));
  const technicalIdentifiers = knowledgeProjection.hidden
    .map((item) => item.detail.match(/^原始技术标识：(.+)$/u)?.[1] || '')
    .filter(Boolean);
  const understanding = {
    summary: `当前资料来源图汇总 ${documentCount} 份资料、${knowledgeTerms.length} 个可信知识词和 ${sectionTitles.length} 个可信结构线索；${readyCount}/${documentCount || 0} 份已进入检索，尚未生成可解释语义快照。`,
    keyConcepts: knowledgeTerms.map((term) => term.display_label),
    keyFields: [],
    technicalIdentifiers,
    structurePath: sectionTitles.map((title) => title.display_label),
    strategies: [],
    retrievalCoverage: { ready: readyCount, total: documentCount },
  };
  const pipeline = [
    {
      key: 'ingest',
      label: '资料接入',
      value: `${documentCount} 份`,
      detail: actualDocumentCount ? '来自当前文档列表' : '来自数据集文档数摘要',
      status: documentCount ? 'complete' : 'empty',
      source: actualDocumentCount ? '文档列表 dataset_id / dataset_ids' : '数据集 document_count 摘要',
      summary: actualDocumentCount
        ? `当前可见文档列表返回 ${actualDocumentCount} 份资料。`
        : `数据集摘要记录 ${documentCount} 份资料，接口未返回文档级清单。`,
      items: documentItems,
    },
    {
      key: 'clean',
      label: '解析清洗',
      value: parsedCount ? `${parsedCount} 已解析` : parseSummary || '待解析',
      detail: attentionCount ? `${attentionCount} 份需要关注` : '未发现质量告警',
      status: attentionCount ? 'attention' : parsedCount ? 'complete' : 'empty',
      source: actualDocumentCount
        ? '文档 parse_status / parse_quality_status / lifecycle'
        : '数据集 parse_status_summary',
      summary: actualDocumentCount
        ? `${parsedCount}/${documentCount} 份已解析，${attentionCount} 份需要关注。`
        : `解析摘要：${parseSummary || '接口未返回'}。`,
      items: parseItems,
    },
    {
      key: 'structure',
      label: '结构识别',
      value: sectionTitles.length ? `${sectionTitles.length} 个结构线索` : '待识别',
      detail: sectionTitles.length
        ? `已返回 ${sectionTitles.length} 个可信中文结构线索`
        : sectionProjection.hidden.length || strategyProjection.hidden.length
          ? '低质量结构或策略标识已进入待解释清单'
          : '接口暂无章节线索',
      status: sectionTitles.length ? 'complete' : sectionProjection.hidden.length || strategyProjection.hidden.length ? 'attention' : 'empty',
      source: '数据集 section_title_hints / document_understanding_strategies',
      summary: `质量门禁保留 ${sectionTitles.length} 个可信中文结构线索，隐藏 ${sectionProjection.hidden.length + strategyProjection.hidden.length} 个技术或低质量项。`,
      items: structureItems,
    },
    {
      key: 'knowledge',
      label: '知识抽取',
      value: knowledgeTerms.length ? `${knowledgeTerms.length} 个可信知识词` : '待抽取',
      detail: knowledgeTerms.length ? '来自通过质量门禁的数据集名词线索' : knowledgeProjection.hidden.length ? '低质量知识线索已进入待解释清单' : '接口暂无知识词',
      status: knowledgeTerms.length ? 'complete' : knowledgeProjection.hidden.length ? 'attention' : 'empty',
      source: '数据集 noun_term_hints',
      summary: `质量门禁保留 ${knowledgeTerms.length} 个可信中文知识词，隐藏 ${knowledgeProjection.hidden.length} 个技术或低质量项。`,
      items: knowledgeItems,
    },
    {
      key: 'ready',
      label: '检索就绪',
      value: `${readyCount} 可检索`,
      detail: documentCount ? `${Math.max(0, documentCount - readyCount)} 份仍在处理或未返回就绪状态` : '暂无文档',
      status: readyCount && readyCount >= documentCount ? 'complete' : readyCount ? 'attention' : 'empty',
      source: actualDocumentCount ? '文档 lifecycle / parse_status' : '数据集 parse_status_summary',
      summary: `${readyCount}/${documentCount || 0} 份资料已进入检索；只展示接口明确返回为 indexed 或 ready 的文档。`,
      items: readyItems,
    },
  ];

  return {
    hasDataset: true,
    mode: 'fallback',
    viewLabel: '资料来源图（语义生成中）',
    overviewTitle: '当前资料来源包含什么',
    snapshotStatus: semanticUnderstanding?.status || 'unavailable',
    statusMessage: semanticUnderstanding?.status === 'empty'
      ? '资料来源图（语义生成中）：语义快照尚未生成，当前只展示通过质量门禁的资料与摘要线索。'
      : '当前展示资料来源图；统一语义快照可用后会自动切换为真实语义图。',
    datasetId,
    title: cleanText(dataset.title) || cleanText(dataset.key) || '当前数据集',
    categories: DATASET_GRAPH_CATEGORIES,
    metrics,
    pipeline,
    understanding,
    nodes,
    links,
    fallbackQuality,
    emptyKnowledgeMessage: knowledgeTerms.length || sectionTitles.length || understandingStrategies.length
      ? ''
      : rawKnowledgeTerms.length || rawSectionTitles.length || rawUnderstandingStrategies.length
        ? '当前摘要中的低质量知识词、结构或策略项已进入待解释清单，未进入主画布。'
        : '当前接口尚未返回知识词、章节或理解策略。',
  };
}
