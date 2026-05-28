import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildDocumentDetailViewModel,
  chunkSectionHints,
  documentRawTextFromChunks,
} from './document-detail-view.js';

test('document detail view model orders chunks and keeps markdown source scroll body intact', () => {
  const chunks = [
    {
      id: 'chunk-2',
      chunk_index: 2,
      content: '## 验收\n上线前完成回归。',
      metadata: { section_title_hints: ['验收'] },
    },
    {
      id: 'chunk-1',
      chunk_index: 1,
      content: '# 概览\n系统边界保持 V3 控制。',
      metadata: { section_title_hints: ['概览', '验收'] },
    },
  ];

  assert.equal(
    documentRawTextFromChunks(chunks),
    '## 验收\n上线前完成回归。\n\n# 概览\n系统边界保持 V3 控制。',
  );

  const viewModel = buildDocumentDetailViewModel({
    documents: [],
    selectedDocumentId: 'doc-1',
    selectedDocumentDetail: {
      document: { id: 'doc-1', dataset_id: 'dataset-a', title: '交接说明' },
      chunks,
      retrieval_evidences: [{ id: 'evidence-1' }],
      model_facing: { evidence_state: 'live_detail' },
    },
  });

  assert.deepEqual(viewModel.orderedChunks.map((chunk) => chunk.id), ['chunk-1', 'chunk-2']);
  assert.equal(
    viewModel.rawText,
    '# 概览\n系统边界保持 V3 控制。\n\n## 验收\n上线前完成回归。',
  );
  assert.deepEqual(viewModel.markdownSectionHints, ['概览', '验收']);
  assert.equal(viewModel.evidences.length, 1);
  assert.equal(viewModel.modelFacing.evidence_state, 'live_detail');
});

test('document detail view model switches previous and next inside the same dataset', () => {
  const documents = [
    { id: 'doc-a', dataset_id: 'dataset-a', title: 'A' },
    { id: 'doc-b', dataset_id: 'dataset-a', title: 'B' },
    { id: 'doc-c', dataset_id: 'dataset-b', title: 'C' },
    { id: 'doc-d', dataset_id: 'dataset-a', title: 'D' },
  ];

  const viewModel = buildDocumentDetailViewModel({
    documents,
    selectedDocumentId: 'doc-b',
    selectedDocumentDetail: null,
  });

  assert.equal(viewModel.selectedDocument.id, 'doc-b');
  assert.equal(viewModel.previousDocument.id, 'doc-a');
  assert.equal(viewModel.nextDocument.id, 'doc-d');
});

test('document detail view model treats secondary dataset memberships as siblings', () => {
  const documents = [
    { id: 'doc-a', dataset_id: 'dataset-a', title: 'A' },
    { id: 'doc-b', dataset_id: 'dataset-b', dataset_ids: ['dataset-b', 'dataset-a'], title: 'B' },
    { id: 'doc-c', dataset_id: 'dataset-c', title: 'C' },
  ];

  const viewModel = buildDocumentDetailViewModel({
    documents,
    selectedDocumentId: 'doc-b',
    selectedDocumentDetail: null,
  });

  assert.equal(viewModel.previousDocument.id, 'doc-a');
  assert.equal(viewModel.nextDocument, null);
});

test('chunk section hints ignore blank values and missing metadata', () => {
  assert.deepEqual(chunkSectionHints({ metadata: { section_title_hints: ['接口', '', '  验收  '] } }), [
    '接口',
    '验收',
  ]);
  assert.deepEqual(chunkSectionHints({ metadata: {} }), []);
  assert.deepEqual(chunkSectionHints(null), []);
});
