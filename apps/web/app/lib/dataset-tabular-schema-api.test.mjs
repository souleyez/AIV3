import assert from 'node:assert/strict';
import test from 'node:test';

import {
  createDatasetTabularSchemaClient,
  normalizeDatasetTabularSchema,
} from './dataset-tabular-schema-api.js';

test('tabular schema normalization preserves table and header order', () => {
  const normalized = normalizeDatasetTabularSchema({
    schema_version: '1.0.0',
    dataset_id: 'dataset-1',
    tables: [{
      document_id: 'document-1',
      title: 'traffic.csv',
      content_type: 'text/csv',
      updated_at: '2026-07-17T00:00:00Z',
      structural_source: 'file_header',
      columns: [
        { ordinal: 2, name: 'trafficIn' },
        { ordinal: 1, name: 'day' },
      ],
    }],
  });

  assert.equal(normalized.datasetId, 'dataset-1');
  assert.equal(normalized.tabularDocumentCount, 1);
  assert.equal(normalized.columnCount, 2);
  assert.deepEqual(normalized.tables[0].columns.map((column) => column.name), ['day', 'trafficIn']);
});

test('tabular schema client forwards local scope headers without exposing storage data', async () => {
  let captured = null;
  const client = createDatasetTabularSchemaClient({
    readSecretBindingIdsHeader: () => 'binding-1',
    readLocalThreadId: () => 'thread-1',
    fetchImpl: async (...args) => {
      captured = args;
      return new Response(JSON.stringify({
        dataset_id: 'dataset / 1',
        tables: [],
      }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    },
  });

  const result = await client('dataset / 1');

  assert.equal(captured[0], '/api/v3/datasets/dataset%20%2F%201/tabular-schema');
  assert.equal(captured[1].headers['X-AI-Data-Platform-Secret-Binding-Ids'], 'binding-1');
  assert.equal(captured[1].headers['X-AI-Data-Platform-Local-Thread-Id'], 'thread-1');
  assert.equal(result.datasetId, 'dataset / 1');
});
