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

test('buildDatasetUnderstandingGraph deduplicates existing knowledge hints and keeps evidence labels', () => {
  const model = buildDatasetUnderstandingGraph(dataset, documents);
  const knowledgeNodes = model.nodes.filter((node) => node.kind === 'knowledge');

  assert.deepEqual(knowledgeNodes.map((node) => node.name), ['客户分层', '流失风险']);
  assert.ok(knowledgeNodes.every((node) => node.evidence === '数据集 noun_term_hints'));
  assert.ok(model.links.some((link) => link.source === 'dataset:dataset-main' && link.target === knowledgeNodes[0].id));
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
  assert.deepEqual(model.nodes, []);
  assert.deepEqual(model.links, []);
});

