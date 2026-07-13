import assert from 'node:assert/strict';
import test from 'node:test';

import { buildDatasetUnderstandingGraph } from './dataset-understanding-graph.js';

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
