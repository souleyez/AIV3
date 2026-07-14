export const DATASET_GRAPH_CATEGORIES = [
  { key: 'dataset', name: '数据集', color: '#f8fbff' },
  { key: 'object', name: '业务对象', color: '#38bdf8' },
  { key: 'field', name: '关键字段', color: '#5eead4' },
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
  if (value.includes('pdf')) return 'PDF';
  if (value.includes('sheet') || value.includes('excel') || value.includes('csv') || /(^|\W)xlsx?(\W|$)/.test(value)) return '表格';
  if (value.includes('word') || value.includes('document') || /(^|\W)docx?(\W|$)/.test(value)) return 'Word 文档';
  if (value.includes('presentation') || value.includes('powerpoint') || /(^|\W)pptx?(\W|$)/.test(value)) return '演示文稿';
  if (value.includes('markdown') || /(^|\W)md(\W|$)/.test(value)) return 'Markdown';
  if (value.includes('html') || value.includes('web') || value.includes('url')) return '网页';
  if (value.startsWith('image/')) return '图片';
  if (value.startsWith('audio/') || value.startsWith('video/')) return '音视频';
  if (value.startsWith('text/')) return '文本';
  return contentType;
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
  } = options;
  const localIds = focusDepth === 'all'
    ? null
    : graphNeighborhoodIds(model, focusNodeId, focusDepth);
  const revealUnresolved = activeCategory === 'unresolved';
  const preferredBusinessFieldIds = new Set();
  if (model.mode === 'semantic' && viewMode === 'business' && !revealUnresolved) {
    const fieldsByObject = new Map();
    model.nodes.filter((node) => node.entityType === 'field' && !node.technicalOnly).forEach((node) => {
      const fields = fieldsByObject.get(node.objectId) || [];
      fields.push(node);
      fieldsByObject.set(node.objectId, fields);
    });
    fieldsByObject.forEach((fields) => fields
      .sort((left, right) => right.businessScore - left.businessScore || left.name.localeCompare(right.name, 'zh-CN'))
      .slice(0, 4)
      .forEach((field) => preferredBusinessFieldIds.add(field.id)));
  }
  const nodes = model.nodes.filter((node) => {
    if (activeCategory !== 'all' && node.kind !== 'dataset' && node.kind !== activeCategory) return false;
    if (
      model.mode === 'semantic'
      && viewMode === 'business'
      && !revealUnresolved
      && node.kind !== 'dataset'
      && (node.kind === 'unresolved' || node.technicalOnly)
    ) return false;
    if (
      model.mode === 'semantic'
      && viewMode === 'business'
      && !revealUnresolved
      && node.entityType === 'field'
      && !preferredBusinessFieldIds.has(node.id)
    ) return false;
    if (localIds && !localIds.has(node.id)) return false;
    return true;
  });
  const visibleIds = new Set(nodes.map((node) => node.id));
  const links = model.links.filter((link) => (
    visibleIds.has(link.source)
      && visibleIds.has(link.target)
      && (activeRelationType === 'all' || link.type === activeRelationType)
  ));
  return { nodes, links };
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
  const knowledgeTerms = cleanList(
    dataset.noun_term_hints || dataset.nounTermHints || dataset.noun_terms || dataset.nounTerms,
    LIMITS.knowledge,
  );
  const sectionTitles = cleanList(
    dataset.section_title_hints || dataset.sectionTitleHints,
    LIMITS.section,
  );
  const understandingStrategies = cleanList(
    dataset.document_understanding_strategies || dataset.documentUnderstandingStrategies,
    LIMITS.strategy,
  );
  const materialHints = cleanList(dataset.material_hints || dataset.materialHints, LIMITS.material);
  const explicitContentTypes = summaryItems(dataset.content_type_summary || dataset.contentTypeSummary);
  const documentContentTypes = cleanList(
    scopedDocuments.map((document) => documentTypeLabel(document.content_type || document.contentType)),
    LIMITS.material,
  );
  const materialTypes = cleanList(
    [...documentContentTypes, ...explicitContentTypes, ...materialHints],
    LIMITS.material,
  );

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

  const documentNodes = scopedDocuments.slice(0, LIMITS.document).map((document) => ({
    id: document.id ? `document:${document.id}` : nodeId('document', document.title),
    name: cleanText(document.title) || cleanText(document.object_key) || '未命名文档',
    detail: `${documentTypeLabel(document.content_type || document.contentType) || '未知格式'} · ${documentParseStatus(document) || documentLifecycle(document) || '状态未知'}`,
    status: documentQualityStatus(document) || documentParseStatus(document) || documentLifecycle(document),
    evidence: '当前数据集文档列表',
    materialName: documentTypeLabel(document.content_type || document.contentType),
  }));
  if (documentNodes.length < LIMITS.document) {
    const knownTitles = new Set(documentNodes.map((item) => item.name.toLocaleLowerCase()));
    cleanList(dataset.document_title_hints || dataset.documentTitleHints, LIMITS.document)
      .filter((title) => !knownTitles.has(title.toLocaleLowerCase()))
      .slice(0, LIMITS.document - documentNodes.length)
      .forEach((title, index) => documentNodes.push({
        id: nodeId('document', title, index),
        name: title,
        detail: '数据集摘要返回的文档标题线索。',
        status: '标题线索',
        evidence: '数据集 document_title_hints',
        materialName: '',
      }));
  }
  const createdDocumentNodes = addConnectedNodes(documentNodes, 'document', (item) => graphNode({ ...item, kind: 'document', symbolSize: 18 }));
  const createdKnowledgeNodes = addConnectedNodes(knowledgeTerms, 'knowledge', (term, index) => graphNode({
    id: nodeId('knowledge', term, index),
    name: term,
    kind: 'knowledge',
    detail: knowledgeSignal(term) === 'concept'
      ? '系统从数据集摘要中识别到的业务概念或知识要点。'
      : '系统从数据集摘要中保留的代码式名称或技术标识。',
    evidence: '数据集 noun_term_hints',
    signal: knowledgeSignal(term),
    symbolSize: knowledgeSignal(term) === 'concept' ? 28 : 16,
  }));
  const createdSectionNodes = addConnectedNodes(sectionTitles, 'section', (title, index) => graphNode({
    id: nodeId('section', title, index),
    name: title,
    kind: 'section',
    detail: '解析过程中识别出的章节或结构标题。',
    evidence: '数据集 section_title_hints',
    symbolSize: 21,
  }));
  const createdMaterialNodes = addConnectedNodes(materialTypes, 'material', (material, index) => graphNode({
    id: nodeId('material', material, index),
    name: material,
    kind: 'material',
    detail: '当前数据集已有的资料类型或材料线索。',
    evidence: explicitContentTypes.length ? '数据集 content_type_summary / material_hints' : '文档 content_type / 数据集 material_hints',
    symbolSize: 18,
  }));
  const createdStrategyNodes = addConnectedNodes(understandingStrategies, 'strategy', (strategy, index) => graphNode({
    id: nodeId('strategy', strategy, index),
    name: strategy,
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
  const documentItems = scopedDocuments.map((document, index) => ({
    id: document.id ? `document:${document.id}` : `document-stage:${index}`,
    nodeId: document.id ? `document:${document.id}` : nodeId('document', document.title, index),
    label: cleanText(document.title) || cleanText(document.object_key) || `未命名文档 ${index + 1}`,
    meta: documentTypeLabel(document.content_type || document.contentType) || '格式未返回',
    detail: readableStatus(document.lifecycle),
    evidence: '当前数据集文档列表',
    status: 'complete',
  }));
  const parseItems = scopedDocuments.map((document, index) => ({
    id: document.id ? `parse:${document.id}` : `parse-stage:${index}`,
    nodeId: document.id ? `document:${document.id}` : nodeId('document', document.title, index),
    label: cleanText(document.title) || cleanText(document.object_key) || `未命名文档 ${index + 1}`,
    meta: parsedDocument(document) ? '已解析' : '待解析',
    detail: attentionDocument(document)
      ? `质量状态：${readableStatus(documentQualityStatus(document), '需要关注')}`
      : `解析状态：${readableStatus(documentParseStatus(document) || documentLifecycle(document))}`,
    evidence: '文档 parse_status / parse_quality_status / lifecycle',
    status: attentionDocument(document) ? 'attention' : parsedDocument(document) ? 'complete' : 'empty',
  }));
  const structureItems = [
    ...sectionTitles.map((title, index) => ({
      id: `section-stage:${index}`,
      nodeId: nodeId('section', title, index),
      label: title,
      meta: '结构线索',
      detail: '解析摘要返回的章节或结构标题',
      evidence: '数据集 section_title_hints',
      status: 'complete',
    })),
    ...understandingStrategies.map((strategy, index) => ({
      id: `strategy-stage:${index}`,
      nodeId: nodeId('strategy', strategy, index),
      label: strategy,
      meta: '理解策略',
      detail: '解析摘要记录的文档理解策略',
      evidence: '数据集 document_understanding_strategies',
      status: 'complete',
    })),
  ];
  const knowledgeItems = knowledgeTerms.map((term, index) => ({
    id: `knowledge-stage:${index}`,
    nodeId: nodeId('knowledge', term, index),
    label: term,
    meta: knowledgeSignal(term) === 'concept' ? '核心概念' : '技术标识',
    detail: knowledgeSignal(term) === 'concept'
      ? '接口返回的业务知识词'
      : '接口返回的代码式或英文标识',
    evidence: '数据集 noun_term_hints',
    status: 'complete',
  }));
  const readyItems = scopedDocuments.filter(readyDocument).map((document, index) => ({
    id: document.id ? `ready:${document.id}` : `ready-stage:${index}`,
    nodeId: document.id ? `document:${document.id}` : nodeId('document', document.title, index),
    label: cleanText(document.title) || cleanText(document.object_key) || `未命名文档 ${index + 1}`,
    meta: '可检索',
    detail: `生命周期：${readableStatus(document.lifecycle)} · 解析：${readableStatus(documentParseStatus(document))}`,
    evidence: '文档 lifecycle / parse_status',
    status: 'complete',
  }));
  const understanding = {
    summary: `系统基于现有接口字段，在 ${documentCount} 份资料中识别出 ${knowledgeTerms.length} 个知识词和 ${sectionTitles.length} 个结构线索；${readyCount}/${documentCount || 0} 份已进入检索。`,
    keyConcepts: knowledgeTerms.filter((term) => knowledgeSignal(term) === 'concept'),
    keyFields: [],
    technicalIdentifiers: knowledgeTerms.filter((term) => knowledgeSignal(term) === 'identifier'),
    structurePath: sectionTitles,
    strategies: understandingStrategies,
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
      value: understandingStrategies.length
        ? `${understandingStrategies.length} 种策略`
        : sectionTitles.length
          ? `${sectionTitles.length} 个章节`
          : '待识别',
      detail: sectionTitles.length ? `已返回 ${sectionTitles.length} 个章节线索` : '接口暂无章节线索',
      status: understandingStrategies.length || sectionTitles.length ? 'complete' : 'empty',
      source: '数据集 section_title_hints / document_understanding_strategies',
      summary: `接口返回 ${sectionTitles.length} 个结构线索和 ${understandingStrategies.length} 种理解策略。`,
      items: structureItems,
    },
    {
      key: 'knowledge',
      label: '知识抽取',
      value: knowledgeTerms.length ? `${knowledgeTerms.length} 个知识词` : '待抽取',
      detail: knowledgeTerms.length ? '来自数据集名词线索' : '接口暂无知识词',
      status: knowledgeTerms.length ? 'complete' : 'empty',
      source: '数据集 noun_term_hints',
      summary: `接口返回 ${knowledgeTerms.length} 个知识词，其中 ${understanding.keyConcepts.length} 个包含明确中文语义，${understanding.technicalIdentifiers.length} 个为技术标识。`,
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
    snapshotStatus: semanticUnderstanding?.status || 'unavailable',
    statusMessage: semanticUnderstanding?.status === 'empty'
      ? '语义快照尚未生成，当前展示由数据集摘要和可见资料组成的基础视图。'
      : '当前展示基础理解视图；统一语义快照可用后会自动切换。',
    datasetId,
    title: cleanText(dataset.title) || cleanText(dataset.key) || '当前数据集',
    categories: DATASET_GRAPH_CATEGORIES,
    metrics,
    pipeline,
    understanding,
    nodes,
    links,
    emptyKnowledgeMessage: knowledgeTerms.length || sectionTitles.length || understandingStrategies.length
      ? ''
      : '当前接口尚未返回知识词、章节或理解策略。',
  };
}
