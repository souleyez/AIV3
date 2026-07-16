import assert from 'node:assert/strict';
import test from 'node:test';

import {
  createDatasetSemanticGraphClient,
  DATASET_SEMANTIC_GRAPH_LIMITS,
  datasetSemanticGraphStateAfterFailure,
  datasetSemanticGraphSelectionKey,
  normalizeDatasetSemanticGraph,
  normalizeDatasetSemanticGraphQuery,
} from './dataset-semantic-graph-api.js';

function node(id, kind, datasetRefs, overrides = {}) {
  return {
    id,
    kind,
    display_label: overrides.display_label || '经营资料',
    dataset_refs: datasetRefs,
    visible_provenance_count: overrides.visible_provenance_count ?? datasetRefs.length,
    ...overrides,
  };
}

function edge(id, sourceId, targetId, overrides = {}) {
  return {
    id,
    source_id: sourceId,
    target_id: targetId,
    relation_type: 'explicit_reference',
    label: '明确引用',
    relation_semantics: 'reference',
    evidence_class: 'confirmed',
    confidence: 1,
    reason: '现有证据记录了明确引用。',
    cross_dataset: true,
    supporting_dataset_ids: ['dataset-a', 'dataset-b'],
    ...overrides,
  };
}

function fixture(overrides = {}) {
  return {
    schema_version: '1.0.0',
    root_dataset_id: 'dataset-a',
    datasets: [
      { id: 'dataset-a', title: '新百项目资料', stale: false },
      { id: 'dataset-b', title: '新百经营分析', stale: false },
    ],
    nodes: [
      node('dataset:dataset-a', 'dataset', ['dataset-a'], { display_label: '新百项目资料' }),
      node('dataset:dataset-b', 'dataset', ['dataset-b'], { display_label: '新百经营分析' }),
      node('shared:document', 'document', ['dataset-a', 'dataset-b'], {
        display_label: '经营分析资料',
        visible_provenance_count: 2,
      }),
      node('d:dataset-b:field:amount', 'field', ['dataset-b'], { display_label: '合同金额' }),
    ],
    edges: [
      edge('edge:membership', 'dataset:dataset-a', 'shared:document', {
        relation_type: 'membership',
        label: '包含资料',
        relation_semantics: 'structure',
        evidence_class: 'observed',
        cross_dataset: false,
        supporting_dataset_ids: ['dataset-a'],
      }),
      edge('edge:reference', 'shared:document', 'd:dataset-b:field:amount'),
    ],
    truncated: { datasets: 0, nodes: 0, edges: 0 },
    stale: false,
    cross_links_status: 'ready',
    ...overrides,
  };
}

test('normalizeDatasetSemanticGraph accepts a scoped v1 graph and keeps inspectable evidence', () => {
  const graph = normalizeDatasetSemanticGraph(fixture());

  assert.equal(graph.schema_version, '1.0.0');
  assert.equal(graph.root_dataset_id, 'dataset-a');
  assert.deepEqual(graph.nodes[2].dataset_refs, ['dataset-a', 'dataset-b']);
  assert.equal(graph.nodes[2].visible_provenance_count, 2);
  assert.equal(graph.edges[1].relation_semantics, 'reference');
  assert.equal(graph.edges[1].evidence_class, 'confirmed');
  assert.deepEqual(graph.edges[1].supporting_dataset_ids, ['dataset-a', 'dataset-b']);
});

test('normalizeDatasetSemanticGraph rejects unknown schema/evidence and missing dataset scope', () => {
  assert.throws(
    () => normalizeDatasetSemanticGraph(fixture({ schema_version: '2.0.0' })),
    /schema_version/,
  );
  assert.throws(
    () => normalizeDatasetSemanticGraph(fixture({
      edges: [edge('edge:bad', 'dataset:dataset-a', 'shared:document', { evidence_class: 'guessed' })],
    })),
    /evidence_class/,
  );
  assert.throws(
    () => normalizeDatasetSemanticGraph(fixture({
      nodes: [node('dataset:dataset-a', 'dataset', [])],
      edges: [],
    })),
    /dataset_refs.*required/,
  );
  assert.throws(
    () => normalizeDatasetSemanticGraph(fixture({
      nodes: [node('dataset:dataset-a', 'dataset', ['hidden-dataset'])],
      edges: [],
    })),
    /unknown dataset/,
  );
  assert.throws(
    () => normalizeDatasetSemanticGraph(fixture({
      edges: [edge('edge:upgraded-similarity', 'dataset:dataset-a', 'shared:document', {
        relation_semantics: 'similarity',
        evidence_class: 'confirmed',
      })],
    })),
    /similarity.*inferred/,
  );
});

test('normalizeDatasetSemanticGraph rejects dangling endpoints and frontend response limits', () => {
  assert.throws(
    () => normalizeDatasetSemanticGraph(fixture({
      edges: [edge('edge:dangling', 'dataset:dataset-a', 'missing-node')],
    })),
    /unknown target_id/,
  );

  const nodes = Array.from(
    { length: DATASET_SEMANTIC_GRAPH_LIMITS.nodes + 1 },
    (_, index) => node(`node:${index}`, 'concept', ['dataset-a']),
  );
  assert.throws(
    () => normalizeDatasetSemanticGraph(fixture({ nodes, edges: [] })),
    /nodes exceeds limit 160/,
  );

  const edges = Array.from(
    { length: DATASET_SEMANTIC_GRAPH_LIMITS.edges + 1 },
    (_, index) => edge(`edge:${index}`, 'dataset:dataset-a', 'shared:document'),
  );
  assert.throws(
    () => normalizeDatasetSemanticGraph(fixture({ edges })),
    /edges exceeds limit 240/,
  );
});

test('query normalization caps the web selection at root plus seven datasets', () => {
  const query = normalizeDatasetSemanticGraphQuery({
    rootDatasetId: 'dataset-a',
    datasetIds: ['dataset-c', 'dataset-b', 'dataset-a', 'dataset-b'],
    autoNeighbors: 3,
    maxNodes: 160,
    maxEdges: 240,
    depth: 1,
  });
  assert.deepEqual(query, {
    root_dataset_id: 'dataset-a',
    dataset_ids: ['dataset-b', 'dataset-c'],
    auto_neighbors: 3,
    max_nodes: 160,
    max_edges: 240,
    depth: 1,
  });

  assert.throws(() => normalizeDatasetSemanticGraphQuery({
    rootDatasetId: 'dataset-a',
    datasetIds: Array.from({ length: 8 }, (_, index) => `dataset-${index}`),
  }), /datasets exceeds limit 8/);
});

test('ETag cache is isolated by sorted selection and request limits', async () => {
  const calls = [];
  const responses = [
    new Response(JSON.stringify(fixture()), {
      status: 200,
      headers: { 'content-type': 'application/json', etag: '"selection-a"' },
    }),
    new Response(JSON.stringify(fixture({
      datasets: [
        { id: 'dataset-a', title: '新百项目资料', stale: false },
        { id: 'dataset-c', title: '招商经营分析', stale: false },
      ],
      nodes: [node('dataset:dataset-a', 'dataset', ['dataset-a'])],
      edges: [],
      cross_links_status: 'empty',
    })), {
      status: 200,
      headers: { 'content-type': 'application/json', etag: '"selection-c"' },
    }),
    new Response(null, { status: 304, headers: { etag: '"selection-a"' } }),
    new Response(JSON.stringify(fixture()), {
      status: 200,
      headers: { 'content-type': 'application/json', etag: '"selection-a-expanded"' },
    }),
  ];
  const client = createDatasetSemanticGraphClient({
    fetchImpl: async (url, options) => {
      calls.push({ url, options, body: JSON.parse(options.body) });
      return responses.shift();
    },
    readSecretBindingIdsHeader: () => 'binding-a',
    readLocalThreadId: () => 'thread-a',
  });

  const selectionA = { rootDatasetId: 'dataset-a', datasetIds: ['dataset-b'], maxNodes: 100, maxEdges: 180 };
  const selectionC = { rootDatasetId: 'dataset-a', datasetIds: ['dataset-c'], maxNodes: 100, maxEdges: 180 };
  const firstA = await client(selectionA);
  await client(selectionC);
  const cachedA = await client({ ...selectionA, datasetIds: ['dataset-b', 'dataset-a'] });
  await client({ ...selectionA, maxNodes: 160, maxEdges: 240 });

  assert.equal(firstA.notModified, false);
  assert.equal(cachedA.notModified, true);
  assert.equal(cachedA.data.nodes[2].id, 'shared:document');
  assert.equal(calls[0].url, '/api/v3/dataset-semantic-graphs/query');
  assert.equal(calls[0].options.headers['If-None-Match'], undefined);
  assert.equal(calls[1].options.headers['If-None-Match'], undefined);
  assert.equal(calls[2].options.headers['If-None-Match'], '"selection-a"');
  assert.equal(calls[3].options.headers['If-None-Match'], undefined);
  assert.deepEqual(calls[0].body.dataset_ids, ['dataset-b']);
  assert.equal(calls[0].options.headers['X-AI-Data-Platform-Secret-Binding-Ids'], 'binding-a');
  assert.equal(calls[0].options.headers['X-AI-Data-Platform-Local-Thread-Id'], 'thread-a');
  assert.notEqual(
    datasetSemanticGraphSelectionKey(normalizeDatasetSemanticGraphQuery(selectionA)),
    datasetSemanticGraphSelectionKey(normalizeDatasetSemanticGraphQuery(selectionC)),
  );
});

test('automatic neighbors cannot be admitted by inferred similarity alone', async () => {
  const similarityOnly = fixture({
    edges: [edge('edge:similarity', 'dataset:dataset-a', 'dataset:dataset-b', {
      relation_type: 'label_similarity',
      label: '相似线索',
      relation_semantics: 'similarity',
      evidence_class: 'inferred',
      confidence: 0.7,
      reason: '标签相近，仅作为线索。',
    })],
  });
  const client = createDatasetSemanticGraphClient({
    fetchImpl: async () => new Response(JSON.stringify(similarityOnly), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    }),
    readSecretBindingIdsHeader: () => '',
    readLocalThreadId: () => '',
  });

  await assert.rejects(
    client({ rootDatasetId: 'dataset-a', datasetIds: [], autoNeighbors: 3 }),
    /automatic neighbor dataset-b has no confirmed or observed evidence/,
  );
});

test('masked access failures clear cached cross graphs while 5xx retains data without availability', () => {
  const cachedData = fixture();
  const current = {
    rootDatasetId: 'dataset-a',
    selectionKey: 'cross|dataset-a|dataset-b',
    status: 'ready',
    data: cachedData,
    error: '',
    available: true,
  };
  for (const status of [401, 403, 404]) {
    const error = new Error(`masked ${status}`);
    error.status = status;
    const failed = datasetSemanticGraphStateAfterFailure(current, {
      rootDatasetId: 'dataset-a',
      error,
    });
    assert.equal(failed.available, false);
    assert.equal(failed.data, null);
    assert.equal(failed.selectionKey, '');
  }

  const serverError = new Error('temporary failure');
  serverError.status = 503;
  const retained = datasetSemanticGraphStateAfterFailure(current, {
    rootDatasetId: 'dataset-a',
    error: serverError,
  });
  assert.equal(retained.available, false);
  assert.equal(retained.data, cachedData);
  assert.equal(retained.selectionKey, current.selectionKey);

  const probeFailure = datasetSemanticGraphStateAfterFailure(current, {
    rootDatasetId: 'dataset-a',
    probe: true,
    error: serverError,
  });
  assert.equal(probeFailure.data, null);
  assert.equal(probeFailure.available, false);
});
