import assert from 'node:assert/strict';
import test from 'node:test';

import {
  DATASET_UNDERSTANDING_LIMITS,
  createDatasetUnderstandingClient,
  datasetUnderstandingSelectionKey,
  normalizeDatasetUnderstanding,
} from './dataset-understanding-api.js';

function fixture(overrides = {}) {
  return {
    schema_version: '1.0.0',
    generation_version: 'semantic_profile_v1',
    status: 'ready',
    dataset: { id: 'dataset-1', title: '新百经营分析' },
    coverage: {
      source_count: 7,
      document_count: 486,
      record_count: 485,
      asset_count: 0,
      retrieval_evidence_count: 485,
      confirmed_fact_count: 0,
      unresolved_field_count: 1,
    },
    summary: { headline: '系统识别到租赁合同与门店。', limitations: [] },
    objects: [{
      id: 'object:lease',
      kind: 'database_table',
      label: '租赁合同',
      technical_name: 'BA_LEASE_CONTRACT',
      description: '租赁合同业务对象',
      label_source: 'source_comment',
      confidence: 0.98,
      coverage_count: 128,
      status: 'confirmed',
      evidence_refs: [{ source_kind: 'database_table', source_id: 'table-1', label: '源表注释' }],
    }],
    fields: [{
      id: 'field:shop-name',
      object_id: 'object:lease',
      label: '门店名称',
      technical_name: 'SHOP_NAME',
      semantic_role: 'name',
      value_type: 'text',
      non_empty_count: 120,
      distinct_count: 32,
      examples: ['新百一店', 'person@example.com', 'postgres://secret'],
      status: 'observed',
      label_source: 'source_comment',
      confidence: 0.94,
      evidence_refs: [{ source_kind: 'database_column', source_id: 'column-1', label: '字段注释' }],
    }],
    relations: [{
      id: 'relation:lease-shop',
      source_id: 'object:lease',
      target_id: 'field:shop-name',
      relation_type: 'contains',
      label: '包含门店',
      evidence_class: 'confirmed',
      confidence: 1,
      reason: '字段明确属于租赁合同',
      evidence_refs: [{ source_kind: 'database_column', source_id: 'column-1', label: '字段归属' }],
    }],
    source_groups: [{
      id: 'source:database',
      kind: 'database_table',
      label: '数据库表',
      object_ids: ['object:lease'],
      coverage_count: 128,
    }],
    pipeline: [{ id: 'profile', label: '结构识别', status: 'complete', detail: '识别 1 个对象', evidence_count: 2 }],
    generated_at: '2026-07-13T12:00:00Z',
    stale: false,
    truncated: { objects: 0, fields: 0, relations: 0 },
    ...overrides,
  };
}

test('normalizeDatasetUnderstanding accepts the bounded contract and removes sensitive examples', () => {
  const normalized = normalizeDatasetUnderstanding(fixture());

  assert.equal(normalized.dataset.title, '新百经营分析');
  assert.deepEqual(normalized.fields[0].examples, ['新百一店']);
  assert.equal(normalized.relations[0].evidence_class, 'confirmed');
  assert.equal(normalized.objects[0].technical_name, 'BA_LEASE_CONTRACT');
});

test('normalizeDatasetUnderstanding rejects over-limit arrays instead of truncating an unsafe payload', () => {
  const objects = Array.from({ length: DATASET_UNDERSTANDING_LIMITS.objects + 1 }, (_, index) => ({
    ...fixture().objects[0],
    id: `object:${index}`,
  }));

  assert.throws(
    () => normalizeDatasetUnderstanding(fixture({ objects })),
    /objects exceeds limit 40/,
  );
});

test('normalizeDatasetUnderstanding rejects missing ids, illegal evidence classes, and dangling endpoints', () => {
  assert.throws(
    () => normalizeDatasetUnderstanding(fixture({ objects: [{ ...fixture().objects[0], id: '' }] })),
    /objects\[0\]\.id/,
  );
  assert.throws(
    () => normalizeDatasetUnderstanding(fixture({
      relations: [{ ...fixture().relations[0], evidence_class: 'guessed' }],
    })),
    /evidence_class/,
  );
  assert.throws(
    () => normalizeDatasetUnderstanding(fixture({
      relations: [{ ...fixture().relations[0], target_id: 'field:missing' }],
    })),
    /unknown target_id/,
  );
});

test('dataset understanding client sends browser scope and ETag, then exposes 304 without parsing a body', async () => {
  const calls = [];
  const client = createDatasetUnderstandingClient({
    fetchImpl: async (url, options) => {
      calls.push({ url, options });
      return new Response(null, { status: 304, headers: { etag: '"fingerprint-1"' } });
    },
    readSecretBindingIdsHeader: () => 'binding-a,binding-b',
    readLocalThreadId: () => 'thread-a',
  });

  const result = await client('dataset /一', { etag: '"fingerprint-1"' });

  assert.equal(result.notModified, true);
  assert.equal(result.etag, '"fingerprint-1"');
  assert.equal(calls[0].url, '/api/v3/datasets/dataset%20%2F%E4%B8%80/understanding');
  assert.equal(calls[0].options.headers['If-None-Match'], '"fingerprint-1"');
  assert.equal(calls[0].options.headers['X-AI-Data-Platform-Secret-Binding-Ids'], 'binding-a,binding-b');
  assert.equal(calls[0].options.headers['X-AI-Data-Platform-Local-Thread-Id'], 'thread-a');
});

test('dataset understanding client normalizes a 200 response and returns its ETag', async () => {
  const client = createDatasetUnderstandingClient({
    fetchImpl: async () => new Response(JSON.stringify(fixture()), {
      status: 200,
      headers: { 'content-type': 'application/json', etag: '"fingerprint-2"' },
    }),
    readSecretBindingIdsHeader: () => '',
    readLocalThreadId: () => '',
  });

  const result = await client('dataset-1');

  assert.equal(result.notModified, false);
  assert.equal(result.etag, '"fingerprint-2"');
  assert.equal(result.selectionKey, 'single:dataset-1');
  assert.equal(result.data.objects[0].label, '租赁合同');
});

test('single-dataset selection keys stay backward compatible and isolated per dataset', () => {
  assert.equal(datasetUnderstandingSelectionKey('dataset-1'), 'single:dataset-1');
  assert.equal(datasetUnderstandingSelectionKey(' dataset-2 '), 'single:dataset-2');
  assert.notEqual(
    datasetUnderstandingSelectionKey('dataset-1'),
    datasetUnderstandingSelectionKey('dataset-2'),
  );
});
