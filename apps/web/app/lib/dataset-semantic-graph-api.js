import { readLocalSecretBindingIdsHeader } from './local-account-state.js';
import { readLocalThreadId } from './local-browser-state.js';

export const DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION = '1.0.0';

export const DATASET_SEMANTIC_GRAPH_LIMITS = Object.freeze({
  datasets: 8,
  nodes: 160,
  edges: 240,
  autoNeighbors: 3,
  depth: 2,
});

const NODE_KINDS = new Set(['dataset', 'document', 'object', 'field', 'concept', 'structure']);
const EVIDENCE_CLASSES = new Set(['confirmed', 'observed', 'inferred']);
const RELATION_SEMANTICS = new Set(['identity', 'reference', 'structure', 'similarity']);
const CROSS_LINK_STATUSES = new Set(['ready', 'building', 'stale', 'empty']);

function text(value) {
  return typeof value === 'string' ? value.trim() : '';
}

function requiredId(value, path) {
  const id = text(value);
  if (!id) throw new TypeError(`${path} is required`);
  return id;
}

function array(value, path) {
  if (!Array.isArray(value)) throw new TypeError(`${path} must be an array`);
  return value;
}

function uniqueIds(value, path, { required = false } = {}) {
  const ids = array(value, path).map((id, index) => requiredId(id, `${path}[${index}]`));
  if (required && !ids.length) throw new TypeError(`${path} is required`);
  if (new Set(ids).size !== ids.length) throw new TypeError(`${path} contains duplicate ids`);
  return ids;
}

function nonNegativeInteger(value, path) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < 0) throw new TypeError(`${path} must be a non-negative integer`);
  return number;
}

function boundedConfidence(value, path) {
  const number = Number(value);
  if (!Number.isFinite(number) || number < 0 || number > 1) {
    throw new TypeError(`${path} must be between 0 and 1`);
  }
  return number;
}

function enumValue(value, allowed, path) {
  const normalized = text(value).toLowerCase();
  if (!allowed.has(normalized)) throw new TypeError(`${path} is invalid`);
  return normalized;
}

function requestInteger(value, fallback, minimum, maximum, path) {
  if (value === undefined || value === null || value === '') return fallback;
  const number = Number(value);
  if (!Number.isInteger(number) || number < minimum || number > maximum) {
    throw new RangeError(`${path} must be between ${minimum} and ${maximum}`);
  }
  return number;
}

function assertLimit(items, key) {
  const limit = DATASET_SEMANTIC_GRAPH_LIMITS[key];
  if (items.length > limit) throw new RangeError(`${key} exceeds limit ${limit}`);
}

function assertKnownDatasetIds(ids, datasetIds, path) {
  ids.forEach((id) => {
    if (!datasetIds.has(id)) throw new TypeError(`${path} references unknown dataset ${id}`);
  });
}

function normalizeDataset(dataset, index) {
  return {
    id: requiredId(dataset?.id, `datasets[${index}].id`),
    title: text(dataset?.title) || '可见数据集',
    stale: dataset?.stale === true,
  };
}

function normalizeNode(node, index, datasetIds) {
  const datasetRefs = uniqueIds(node?.dataset_refs, `nodes[${index}].dataset_refs`, { required: true });
  assertKnownDatasetIds(datasetRefs, datasetIds, `nodes[${index}].dataset_refs`);
  return {
    id: requiredId(node?.id, `nodes[${index}].id`),
    kind: enumValue(node?.kind, NODE_KINDS, `nodes[${index}].kind`),
    display_label: text(node?.display_label) || '待解释节点',
    dataset_refs: datasetRefs,
    visible_provenance_count: nonNegativeInteger(
      node?.visible_provenance_count,
      `nodes[${index}].visible_provenance_count`,
    ),
  };
}

function normalizeEdge(edge, index, nodeIds, datasetIds) {
  const sourceId = requiredId(edge?.source_id, `edges[${index}].source_id`);
  const targetId = requiredId(edge?.target_id, `edges[${index}].target_id`);
  if (!nodeIds.has(sourceId)) throw new TypeError(`edges[${index}] references unknown source_id`);
  if (!nodeIds.has(targetId)) throw new TypeError(`edges[${index}] references unknown target_id`);
  const supportingDatasetIds = uniqueIds(
    edge?.supporting_dataset_ids,
    `edges[${index}].supporting_dataset_ids`,
    { required: true },
  );
  assertKnownDatasetIds(supportingDatasetIds, datasetIds, `edges[${index}].supporting_dataset_ids`);
  if (typeof edge?.cross_dataset !== 'boolean') {
    throw new TypeError(`edges[${index}].cross_dataset must be a boolean`);
  }
  const relationSemantics = enumValue(
    edge?.relation_semantics,
    RELATION_SEMANTICS,
    `edges[${index}].relation_semantics`,
  );
  const evidenceClass = enumValue(
    edge?.evidence_class,
    EVIDENCE_CLASSES,
    `edges[${index}].evidence_class`,
  );
  if (relationSemantics === 'similarity' && evidenceClass !== 'inferred') {
    throw new TypeError(`edges[${index}] similarity evidence must remain inferred`);
  }
  return {
    id: requiredId(edge?.id, `edges[${index}].id`),
    source_id: sourceId,
    target_id: targetId,
    relation_type: text(edge?.relation_type) || 'related_to',
    label: text(edge?.label) || '关联线索',
    relation_semantics: relationSemantics,
    evidence_class: evidenceClass,
    confidence: boundedConfidence(edge?.confidence, `edges[${index}].confidence`),
    reason: text(edge?.reason) || '接口未返回匹配依据。',
    cross_dataset: edge.cross_dataset,
    supporting_dataset_ids: supportingDatasetIds,
  };
}

export function normalizeDatasetSemanticGraph(payload) {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    throw new TypeError('dataset semantic graph payload must be an object');
  }
  const schemaVersion = text(payload.schema_version);
  if (schemaVersion !== DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION) {
    throw new TypeError(`schema_version must be ${DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION}`);
  }

  const datasetsInput = array(payload.datasets, 'datasets');
  const nodesInput = array(payload.nodes, 'nodes');
  const edgesInput = array(payload.edges, 'edges');
  assertLimit(datasetsInput, 'datasets');
  assertLimit(nodesInput, 'nodes');
  assertLimit(edgesInput, 'edges');

  const datasets = datasetsInput.map(normalizeDataset);
  const datasetIds = new Set(datasets.map((dataset) => dataset.id));
  if (datasetIds.size !== datasets.length) throw new TypeError('datasets contain duplicate ids');
  const rootDatasetId = requiredId(payload.root_dataset_id, 'root_dataset_id');
  if (!datasetIds.has(rootDatasetId)) throw new TypeError('root_dataset_id references unknown dataset');

  const nodes = nodesInput.map((node, index) => normalizeNode(node, index, datasetIds));
  const nodeIds = new Set(nodes.map((node) => node.id));
  if (nodeIds.size !== nodes.length) throw new TypeError('nodes contain duplicate ids');
  const edges = edgesInput.map((edge, index) => normalizeEdge(edge, index, nodeIds, datasetIds));
  if (new Set(edges.map((edge) => edge.id)).size !== edges.length) {
    throw new TypeError('edges contain duplicate ids');
  }

  const crossLinksStatus = enumValue(
    payload.cross_links_status,
    CROSS_LINK_STATUSES,
    'cross_links_status',
  );
  return {
    schema_version: schemaVersion,
    generation_version: text(payload.generation_version),
    root_dataset_id: rootDatasetId,
    datasets,
    nodes,
    edges,
    truncated: {
      datasets: nonNegativeInteger(payload.truncated?.datasets ?? 0, 'truncated.datasets'),
      nodes: nonNegativeInteger(payload.truncated?.nodes ?? 0, 'truncated.nodes'),
      edges: nonNegativeInteger(payload.truncated?.edges ?? 0, 'truncated.edges'),
    },
    stale: payload.stale === true,
    cross_links_status: crossLinksStatus,
  };
}

export function normalizeDatasetSemanticGraphQuery(request = {}) {
  const rootDatasetId = requiredId(
    request.rootDatasetId ?? request.root_dataset_id,
    'rootDatasetId',
  );
  const inputDatasetIds = request.datasetIds ?? request.dataset_ids ?? [];
  const datasetIds = [...new Set(array(inputDatasetIds, 'datasetIds')
    .map((id, index) => requiredId(id, `datasetIds[${index}]`)))]
    .filter((id) => id !== rootDatasetId)
    .sort((left, right) => left.localeCompare(right, 'en'));
  if (datasetIds.length + 1 > DATASET_SEMANTIC_GRAPH_LIMITS.datasets) {
    throw new RangeError(`datasets exceeds limit ${DATASET_SEMANTIC_GRAPH_LIMITS.datasets}`);
  }
  return {
    root_dataset_id: rootDatasetId,
    dataset_ids: datasetIds,
    auto_neighbors: requestInteger(
      request.autoNeighbors ?? request.auto_neighbors,
      DATASET_SEMANTIC_GRAPH_LIMITS.autoNeighbors,
      0,
      DATASET_SEMANTIC_GRAPH_LIMITS.autoNeighbors,
      'autoNeighbors',
    ),
    max_nodes: requestInteger(
      request.maxNodes ?? request.max_nodes,
      DATASET_SEMANTIC_GRAPH_LIMITS.nodes,
      1,
      DATASET_SEMANTIC_GRAPH_LIMITS.nodes,
      'maxNodes',
    ),
    max_edges: requestInteger(
      request.maxEdges ?? request.max_edges,
      DATASET_SEMANTIC_GRAPH_LIMITS.edges,
      0,
      DATASET_SEMANTIC_GRAPH_LIMITS.edges,
      'maxEdges',
    ),
    depth: requestInteger(request.depth, 1, 1, DATASET_SEMANTIC_GRAPH_LIMITS.depth, 'depth'),
  };
}

export function datasetSemanticGraphSelectionKey(query) {
  const normalized = normalizeDatasetSemanticGraphQuery(query);
  return [
    'cross',
    normalized.root_dataset_id,
    normalized.dataset_ids.join(','),
    `auto:${normalized.auto_neighbors}`,
    `nodes:${normalized.max_nodes}`,
    `edges:${normalized.max_edges}`,
    `depth:${normalized.depth}`,
  ].join('|');
}

function assertResponseMatchesQuery(graph, query) {
  if (graph.root_dataset_id !== query.root_dataset_id) {
    throw new TypeError('response root_dataset_id does not match request');
  }
  const returnedDatasetIds = new Set(graph.datasets.map((dataset) => dataset.id));
  query.dataset_ids.forEach((datasetId) => {
    if (!returnedDatasetIds.has(datasetId)) {
      throw new TypeError(`response omitted explicitly selected dataset ${datasetId}`);
    }
  });
  if (graph.datasets.length > 1 + query.dataset_ids.length + query.auto_neighbors) {
    throw new RangeError('response returned more automatic neighbors than requested');
  }
  const explicitIds = new Set([query.root_dataset_id, ...query.dataset_ids]);
  graph.datasets.forEach((dataset) => {
    if (explicitIds.has(dataset.id)) return;
    const reliable = graph.edges.some((edge) => (
      edge.cross_dataset
      && edge.evidence_class !== 'inferred'
      && edge.supporting_dataset_ids.includes(dataset.id)
    ));
    if (!reliable) {
      throw new TypeError(`automatic neighbor ${dataset.id} has no confirmed or observed evidence`);
    }
  });
}

async function responseError(response) {
  const contentType = response.headers.get('content-type') || '';
  const payload = contentType.includes('application/json')
    ? await response.json()
    : await response.text();
  const message = typeof payload === 'string'
    ? payload
    : payload?.message || payload?.error || `Request failed: ${response.status}`;
  const error = new Error(message);
  error.status = response.status;
  error.payload = payload;
  return error;
}

export function datasetSemanticGraphStateAfterFailure(current = {}, options = {}) {
  const probe = options.probe === true;
  const error = options.error;
  const status = Number(error?.status) || 0;
  const accessRevoked = [401, 403, 404].includes(status);
  const clearCachedGraph = probe || accessRevoked;
  return {
    rootDatasetId: text(options.rootDatasetId),
    selectionKey: clearCachedGraph ? '' : text(current.selectionKey),
    status: 'failed',
    data: clearCachedGraph ? null : current.data || null,
    error: error instanceof Error ? error.message : '跨数据集语义图谱加载失败',
    available: false,
  };
}

export function createDatasetSemanticGraphClient(dependencies = {}) {
  const {
    fetchImpl = (...args) => globalThis.fetch(...args),
    readSecretBindingIdsHeader = readLocalSecretBindingIdsHeader,
    readLocalThreadId: readThreadId = readLocalThreadId,
  } = dependencies;
  const cache = new Map();

  return async function fetchDatasetSemanticGraph(request, options = {}) {
    const query = normalizeDatasetSemanticGraphQuery(request);
    const selectionKey = datasetSemanticGraphSelectionKey(query);
    const secretBindingIds = text(readSecretBindingIdsHeader());
    const localThreadId = text(readThreadId());
    const scopeKey = `${selectionKey}|bindings:${secretBindingIds}|thread:${localThreadId}`;
    const cached = cache.get(scopeKey) || null;
    const response = await fetchImpl('/api/v3/dataset-semantic-graphs/query', {
      method: 'POST',
      cache: 'no-store',
      credentials: 'include',
      signal: options.signal,
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        ...(secretBindingIds ? { 'X-AI-Data-Platform-Secret-Binding-Ids': secretBindingIds } : {}),
        'X-AI-Data-Platform-Local-Thread-Id': localThreadId,
        ...(cached?.etag ? { 'If-None-Match': cached.etag } : {}),
      },
      body: JSON.stringify(query),
    });
    const etag = response.headers.get('etag') || cached?.etag || '';
    if (response.status === 304) {
      if (!cached?.data) throw new Error('跨数据集图谱返回 304，但当前选择没有可复用缓存。');
      return { data: cached.data, etag, notModified: true, selectionKey };
    }
    if (!response.ok) throw await responseError(response);
    const data = normalizeDatasetSemanticGraph(await response.json());
    assertResponseMatchesQuery(data, query);
    cache.set(scopeKey, { data, etag });
    return { data, etag, notModified: false, selectionKey };
  };
}

export const fetchDatasetSemanticGraph = createDatasetSemanticGraphClient();
