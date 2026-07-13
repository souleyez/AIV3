export const DATASET_GRAPH_CATEGORIES = [
  { key: 'dataset', name: '数据集', color: '#f8fbff' },
  { key: 'document', name: '原始文档', color: '#5eead4' },
  { key: 'knowledge', name: '知识词', color: '#fbbf24' },
  { key: 'section', name: '章节结构', color: '#93c5fd' },
  { key: 'material', name: '资料类型', color: '#c4b5fd' },
  { key: 'strategy', name: '理解策略', color: '#fb7185' },
];

const LIMITS = {
  document: 18,
  knowledge: 14,
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

function graphNode({ id, name, kind, detail, evidence, status = '', symbolSize = 30 }) {
  return {
    id,
    name,
    kind,
    categoryKey: kind,
    category: categoryIndex(kind),
    detail,
    evidence,
    status,
    symbolSize,
    value: 1,
  };
}

function selectionModel() {
  return {
    hasDataset: false,
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
      crossNodeRelationCount: 0,
    },
    pipeline: [],
    nodes: [],
    links: [],
    emptyKnowledgeMessage: '',
  };
}

export function buildDatasetUnderstandingGraph(dataset, documents = []) {
  if (!dataset?.id) return selectionModel();

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
    detail: '系统在数据集摘要中保留的名词或业务知识线索。',
    evidence: '数据集 noun_term_hints',
    symbolSize: 26,
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
  const pipeline = [
    {
      key: 'ingest',
      label: '资料接入',
      value: `${documentCount} 份`,
      detail: actualDocumentCount ? '来自当前文档列表' : '来自数据集文档数摘要',
      status: documentCount ? 'complete' : 'empty',
    },
    {
      key: 'clean',
      label: '解析清洗',
      value: parsedCount ? `${parsedCount} 已解析` : parseSummary || '待解析',
      detail: attentionCount ? `${attentionCount} 份需要关注` : '未发现质量告警',
      status: attentionCount ? 'attention' : parsedCount ? 'complete' : 'empty',
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
    },
    {
      key: 'knowledge',
      label: '知识抽取',
      value: knowledgeTerms.length ? `${knowledgeTerms.length} 个知识词` : '待抽取',
      detail: knowledgeTerms.length ? '来自数据集名词线索' : '接口暂无知识词',
      status: knowledgeTerms.length ? 'complete' : 'empty',
    },
    {
      key: 'ready',
      label: '检索就绪',
      value: `${readyCount} 可检索`,
      detail: documentCount ? `${Math.max(0, documentCount - readyCount)} 份仍在处理或未返回就绪状态` : '暂无文档',
      status: readyCount && readyCount >= documentCount ? 'complete' : readyCount ? 'attention' : 'empty',
    },
  ];

  return {
    hasDataset: true,
    datasetId,
    title: cleanText(dataset.title) || cleanText(dataset.key) || '当前数据集',
    categories: DATASET_GRAPH_CATEGORIES,
    metrics,
    pipeline,
    nodes,
    links,
    emptyKnowledgeMessage: knowledgeTerms.length || sectionTitles.length || understandingStrategies.length
      ? ''
      : '当前接口尚未返回知识词、章节或理解策略。',
  };
}
