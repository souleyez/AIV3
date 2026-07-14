import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildDatasetUnderstandingGraph,
  filterDatasetUnderstandingGraph,
  graphNeighborhoodIds,
} from './dataset-understanding-graph.js';

const dataset = {
  id: 'dataset-main',
  key: 'customer-ops',
  title: '客户经营资料',
  document_count: 3,
  estimated_word_count: 4200,
  parse_status_summary: 'completed:2,pending:1',
  content_type_summary: 'pdf:2,spreadsheet:1',
  document_title_hints: ['客户清单.pdf', '季度复盘.xlsx'],
  material_hints: ['经营材料', '客户反馈'],
  noun_term_hints: ['客户分层', '流失风险', '客户分层'],
  section_title_hints: ['经营概览', '风险跟进'],
  document_understanding_strategies: ['heading', 'table'],
};

const documents = [
  {
    id: 'document-a',
    dataset_id: 'dataset-main',
    dataset_ids: ['dataset-main'],
    title: '客户清单.pdf',
    content_type: 'application/pdf',
    lifecycle: 'indexed',
    parse_status: 'completed',
    parse_quality_status: 'ok',
  },
  {
    id: 'document-b',
    dataset_id: 'dataset-main',
    dataset_ids: ['dataset-main'],
    title: '季度复盘.xlsx',
    content_type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    lifecycle: 'received',
    parse_status: 'completed',
    parse_quality_status: 'attention_required',
  },
  {
    id: 'document-other',
    dataset_id: 'dataset-other',
    dataset_ids: ['dataset-other'],
    title: '不属于当前数据集.pdf',
    content_type: 'application/pdf',
    lifecycle: 'indexed',
    parse_status: 'completed',
    parse_quality_status: 'ok',
  },
];

test('buildDatasetUnderstandingGraph scopes documents and exposes honest pipeline counts', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents);

  assert.equal(model.hasDataset, true);
  assert.equal(model.metrics.documentCount, 2);
  assert.equal(model.metrics.estimatedWordCount, 4200);
  assert.equal(model.metrics.readyDocumentCount, 1);
  assert.equal(model.metrics.attentionDocumentCount, 1);

  const documentNodes = model.nodes.filter((node) => node.kind === 'document');
  assert.deepEqual(documentNodes.map((node) => node.name), ['客户清单.pdf', '季度复盘.xlsx']);
  assert.equal(model.nodes.some((node) => node.name === '不属于当前数据集.pdf'), false);

  assert.equal(model.pipeline.find((stage) => stage.key === 'ingest').value, '2 份');
  assert.equal(model.pipeline.find((stage) => stage.key === 'clean').value, '2 已解析');
  assert.equal(model.pipeline.find((stage) => stage.key === 'ready').value, '1 可检索');
});

test('buildDatasetUnderstandingGraph gives every node a main label capped at five characters', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents);

  assert.ok(model.nodes.length > 0);
  assert.ok(model.nodes.every((node) => node.shortLabel));
  assert.ok(model.nodes.every((node) => Array.from(node.shortLabel).length <= 5));
  assert.equal(model.nodes.find((node) => node.kind === 'dataset')?.shortLabel, '客户经营资');
  assert.equal(model.nodes.find((node) => node.name === '客户清单.pdf')?.shortLabel, '客户清单');
  assert.equal(model.nodes.find((node) => node.name === '季度复盘.xlsx')?.shortLabel, '季度复盘');
});

test('buildDatasetUnderstandingGraph exposes evidence-backed details for every processing stage', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents);
  const stages = Object.fromEntries(model.pipeline.map((stage) => [stage.key, stage]));

  assert.deepEqual(model.pipeline.map((stage) => stage.key), ['ingest', 'clean', 'structure', 'knowledge', 'ready']);
  assert.ok(model.pipeline.every((stage) => stage.source));
  assert.ok(model.pipeline.every((stage) => stage.summary));
  assert.ok(model.pipeline.every((stage) => Array.isArray(stage.items)));
  assert.deepEqual(stages.ingest.items.map((item) => item.label), ['客户清单.pdf', '季度复盘.xlsx']);
  assert.deepEqual(stages.clean.items.map((item) => item.label), ['客户清单.pdf', '季度复盘.xlsx']);
  assert.deepEqual(stages.structure.items.map((item) => item.label), ['经营概览', '风险跟进', 'heading', 'table']);
  assert.deepEqual(stages.knowledge.items.map((item) => item.label), ['客户分层', '流失风险']);
  assert.deepEqual(stages.ready.items.map((item) => item.label), ['客户清单.pdf']);
  assert.match(stages.structure.source, /section_title_hints/);
  assert.match(stages.knowledge.source, /noun_term_hints/);
});

test('buildDatasetUnderstandingGraph separates core concepts from technical identifiers without inventing topics', () => {
  const model = buildDatasetUnderstandingGraph({
    ...dataset,
    noun_term_hints: ['k11', 'lcrm', '客户分层', '流失风险', '代码'],
  }, documents);

  assert.deepEqual(model.understanding.keyConcepts, ['客户分层', '流失风险', '代码']);
  assert.deepEqual(model.understanding.technicalIdentifiers, ['k11', 'lcrm']);
  assert.deepEqual(model.understanding.structurePath, ['经营概览', '风险跟进']);
  assert.deepEqual(model.understanding.retrievalCoverage, { ready: 1, total: 2 });
  assert.ok(model.understanding.summary.includes('5 个知识词'));
  assert.ok(model.nodes.filter((node) => node.kind === 'knowledge' && node.signal === 'concept').every((node) => node.symbolSize > 20));
  assert.ok(model.nodes.filter((node) => node.kind === 'knowledge' && node.signal === 'identifier').every((node) => node.symbolSize < 20));
});

test('buildDatasetUnderstandingGraph deduplicates existing knowledge hints and keeps evidence labels', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents);
  const knowledgeNodes = model.nodes.filter((node) => node.kind === 'knowledge');

  assert.deepEqual(knowledgeNodes.map((node) => node.name), ['客户分层', '流失风险']);
  assert.ok(knowledgeNodes.every((node) => node.evidence === '数据集 noun_term_hints'));
  assert.ok(model.links.some((link) => link.source === 'dataset:dataset-main' && link.target === knowledgeNodes[0].id));
});

test('buildDatasetUnderstandingGraph adds factual document-to-material relationships', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents);
  const pdfNode = model.nodes.find((node) => node.kind === 'material' && node.name === 'PDF');
  const spreadsheetNode = model.nodes.find((node) => node.kind === 'material' && node.name === '表格');

  assert.ok(pdfNode);
  assert.ok(spreadsheetNode);
  assert.ok(model.links.some((link) => (
    link.source === 'document:document-a'
      && link.target === pdfNode.id
      && link.type === 'observed'
      && link.relation === '资料格式'
      && link.confidence === 1
      && link.evidence === '文档 content_type'
  )));
  assert.ok(model.links.some((link) => (
    link.source === 'document:document-b'
      && link.target === spreadsheetNode.id
      && link.type === 'observed'
  )));
});

test('buildDatasetUnderstandingGraph creates sparse inferred knowledge and lexical relationships with evidence', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents);
  const knowledgeGroupLink = model.links.find((link) => (
    link.source === 'knowledge:客户分层'
      && link.target === 'knowledge:流失风险'
      && link.relation === '同组线索'
  ));
  const lexicalLink = model.links.find((link) => (
    link.source === 'document:document-a'
      && link.target === 'knowledge:客户分层'
      && link.relation === '词义线索'
  ));

  assert.equal(knowledgeGroupLink?.type, 'inferred');
  assert.ok(knowledgeGroupLink.confidence > 0 && knowledgeGroupLink.confidence < 1);
  assert.match(knowledgeGroupLink.evidence, /noun_term_hints/);
  assert.equal(lexicalLink?.type, 'inferred');
  assert.ok(lexicalLink.confidence >= 0.5 && lexicalLink.confidence < 1);
  assert.match(lexicalLink.evidence, /文本片段/);
  assert.ok(model.metrics.observedRelationCount > 0);
  assert.ok(model.metrics.inferredRelationCount > 0);
  assert.ok(model.metrics.crossNodeRelationCount > 0);
});

test('buildDatasetUnderstandingGraph deduplicates relation pairs and gives every link an inspectable contract', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents);
  const pairKeys = model.links.map((link) => [link.source, link.target].sort().join('|'));

  assert.equal(new Set(pairKeys).size, pairKeys.length);
  assert.ok(model.links.every((link) => link.id));
  assert.ok(model.links.every((link) => ['observed', 'inferred'].includes(link.type)));
  assert.ok(model.links.every((link) => typeof link.confidence === 'number'));
  assert.ok(model.links.every((link) => link.evidence));
});

test('buildDatasetUnderstandingGraph does not fabricate knowledge or strategy nodes for missing fields', () => {
  const model = buildDatasetUnderstandingGraph(
    { id: 'dataset-empty', key: 'empty', title: '空白资料库' },
    [{
      id: 'document-empty',
      dataset_id: 'dataset-empty',
      title: '原始资料.txt',
      content_type: 'text/plain',
      lifecycle: 'received',
      parse_status: 'pending',
    }],
  );

  assert.equal(model.nodes.some((node) => node.kind === 'knowledge'), false);
  assert.equal(model.nodes.some((node) => node.kind === 'section'), false);
  assert.equal(model.nodes.some((node) => node.kind === 'strategy'), false);
  assert.equal(model.emptyKnowledgeMessage, '当前接口尚未返回知识词、章节或理解策略。');
});

test('buildDatasetUnderstandingGraph returns a selection state without a dataset', () => {
  const model = buildDatasetUnderstandingGraph(null, documents);

  assert.equal(model.hasDataset, false);
  assert.equal(model.metrics.observedRelationCount, 0);
  assert.equal(model.metrics.inferredRelationCount, 0);
  assert.equal(model.metrics.crossNodeRelationCount, 0);
  assert.deepEqual(model.nodes, []);
  assert.deepEqual(model.links, []);
});

const semanticUnderstanding = {
  schema_version: '1.0.0',
  generation_version: 'semantic_profile_v1',
  status: 'ready',
  dataset: { id: 'dataset-main', title: '新百经营分析' },
  coverage: {
    source_count: 7,
    document_count: 486,
    record_count: 485,
    asset_count: 0,
    retrieval_evidence_count: 485,
    confirmed_fact_count: 0,
    unresolved_field_count: 1,
  },
  summary: {
    headline: '系统识别到租赁合同、门店与租售明细。',
    limitations: ['尚无已确认事实快照。'],
  },
  objects: [
    {
      id: 'object:lease',
      kind: 'database_table',
      label: '租赁合同',
      technical_name: 'BA_LEASE_CONTRACT',
      description: '承载合同与门店租赁信息',
      label_source: 'source_comment',
      confidence: 0.98,
      coverage_count: 320,
      status: 'confirmed',
      evidence_refs: [{ source_kind: 'database_table', source_id: 'table:lease', label: '源表注释' }],
    },
    {
      id: 'object:shop',
      kind: 'database_table',
      label: '门店',
      technical_name: 'BA_SHOP',
      description: '门店主数据',
      label_source: 'dictionary',
      confidence: 1,
      coverage_count: 85,
      status: 'confirmed',
      evidence_refs: [{ source_kind: 'semantic_dictionary', source_id: 'dictionary:shop', label: '人工确认' }],
    },
  ],
  fields: [
    {
      id: 'field:shop-name',
      object_id: 'object:shop',
      label: '门店名称',
      technical_name: 'SHOP_NAME',
      semantic_role: 'name',
      value_type: 'text',
      non_empty_count: 84,
      distinct_count: 80,
      examples: ['新百一店'],
      status: 'observed',
      label_source: 'source_comment',
      confidence: 0.94,
      evidence_refs: [{ source_kind: 'database_column', source_id: 'column:shop-name', label: '字段注释' }],
    },
    {
      id: 'field:code',
      object_id: 'object:lease',
      label: 'CARDPARENTNAME',
      technical_name: 'CARDPARENTNAME',
      semantic_role: 'unknown',
      value_type: 'text',
      non_empty_count: 300,
      distinct_count: 4,
      examples: ['A'],
      status: 'unresolved',
      label_source: 'raw_identifier',
      confidence: 0,
      evidence_refs: [{ source_kind: 'database_column', source_id: 'column:code', label: '原始字段' }],
    },
  ],
  relations: [{
    id: 'relation:lease-shop',
    source_id: 'object:lease',
    target_id: 'object:shop',
    relation_type: 'shared_key',
    label: '关联门店',
    evidence_class: 'observed',
    confidence: 0.91,
    reason: '共享门店编号，覆盖率与值域重叠通过门禁',
    evidence_refs: [{ source_kind: 'database_profile', source_id: 'profile:1', label: '值域重叠' }],
  }],
  source_groups: [{
    id: 'source:database',
    kind: 'database_table',
    label: '经营数据库',
    object_ids: ['object:lease', 'object:shop'],
    coverage_count: 405,
  }],
  pipeline: [{ id: 'profile', label: '结构识别', status: 'complete', detail: '识别 2 个业务对象', evidence_count: 5 }],
  generated_at: '2026-07-13T12:00:00Z',
  stale: false,
  truncated: { objects: 0, fields: 0, relations: 0 },
};

test('semantic snapshot is the primary graph input and keeps technical names out of main labels', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents, semanticUnderstanding);

  assert.equal(model.mode, 'semantic');
  assert.equal(model.nodes.some((node) => node.name === '客户清单.pdf'), false);
  assert.ok(model.nodes.some((node) => node.kind === 'object' && node.name === '租赁合同'));
  assert.ok(model.nodes.some((node) => node.kind === 'field' && node.name === '门店名称'));
  assert.ok(model.nodes.some((node) => node.kind === 'unresolved' && node.name === '待解释字段'));
  assert.ok(model.nodes.every((node) => node.name !== 'BA_LEASE_CONTRACT'));
  assert.equal(model.understanding.technicalIdentifiers.includes('CARDPARENTNAME'), true);
});

test('semantic graph forms object-field neighborhoods and only uses backend relations for cross-object meaning', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents, semanticUnderstanding);
  const objectRelation = model.links.find((link) => link.id === 'relation:lease-shop');
  const fieldMembership = model.links.find((link) => (
    link.source === 'object:shop' && link.target === 'field:shop-name'
  ));

  assert.equal(objectRelation?.type, 'observed');
  assert.equal(objectRelation?.relation, '关联门店');
  assert.match(objectRelation?.evidence, /共享门店编号/);
  assert.equal(fieldMembership?.structural, true);
  assert.equal(model.links.some((link) => link.relation === '同组线索'), false);
});

test('semantic graph exposes incoming and outgoing neighborhoods for local focus', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents, semanticUnderstanding);
  const lease = model.nodes.find((node) => node.id === 'object:lease');
  const shop = model.nodes.find((node) => node.id === 'object:shop');

  assert.ok(lease.outgoing.some((item) => item.nodeId === 'object:shop'));
  assert.ok(shop.incoming.some((item) => item.nodeId === 'object:lease'));
  assert.ok(shop.outgoing.some((item) => item.nodeId === 'field:shop-name'));
});

test('empty semantic status falls back honestly to existing dataset hints', () => {
  const empty = {
    ...semanticUnderstanding,
    status: 'empty',
    objects: [],
    fields: [],
    relations: [],
  };
  const model = buildDatasetUnderstandingGraph(dataset, documents, empty);

  assert.equal(model.mode, 'fallback');
  assert.equal(model.snapshotStatus, 'empty');
  assert.ok(model.nodes.some((node) => node.name === '客户清单.pdf'));
  assert.match(model.statusMessage, /基础视图/);
});

test('local graph focus returns stable one-hop and two-hop neighborhoods', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents, semanticUnderstanding);

  const oneHop = graphNeighborhoodIds(model, 'object:lease', 1);
  const twoHop = graphNeighborhoodIds(model, 'object:lease', 2);

  assert.deepEqual([...oneHop].sort(), [
    'dataset:dataset-main',
    'field:code',
    'object:lease',
    'object:shop',
  ]);
  assert.ok(twoHop.has('field:shop-name'));
  assert.ok(twoHop.size > oneHop.size);
});

test('business view hides technical noise while an explicit unresolved filter can reveal it', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents, semanticUnderstanding);
  const business = filterDatasetUnderstandingGraph(model, {
    viewMode: 'business',
    activeCategory: 'all',
    activeRelationType: 'all',
    focusDepth: 'all',
  });
  const unresolved = filterDatasetUnderstandingGraph(model, {
    viewMode: 'business',
    activeCategory: 'unresolved',
    activeRelationType: 'all',
    focusDepth: 'all',
  });

  assert.equal(business.nodes.some((node) => node.id === 'field:code'), false);
  assert.equal(unresolved.nodes.some((node) => node.id === 'field:code'), true);
  assert.ok(business.links.every((link) => (
    business.nodes.some((node) => node.id === link.source)
      && business.nodes.some((node) => node.id === link.target)
  )));
});
